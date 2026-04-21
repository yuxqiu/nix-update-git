use std::io::Read;

use anyhow::{Context, Result};
use globset::GlobBuilder;
use nix_prefetch_git::NarHash;
use patchkit::unified::{HunkLine, PlainOrBinaryPatch, UnifiedPatch, parse_patches};

/// Options controlling fetchpatch normalization.
///
/// These correspond to the options that nixpkgs' `fetchpatch` passes
/// to `filterdiff` and related tools during patch normalization.
#[derive(Default)]
pub struct PatchOptions {
    /// Number of path components to strip from all file paths.
    /// Corresponds to `stripLen` in nixpkgs (default 0).
    pub strip_len: usize,
    /// When set, only include files whose path (after stripping one
    /// component) starts with `${relative}/`. The effective strip count
    /// is increased by `1 + count_of_path_segments(relative)`.
    /// Corresponds to `relative` in nixpkgs.
    pub relative: Option<String>,
    /// When set, prepend `a/${extra_prefix}/` to old paths and
    /// `b/${extra_prefix}/` to new paths after stripping.
    /// Corresponds to `extraPrefix` in nixpkgs.
    pub extra_prefix: Option<String>,
    /// Glob patterns for files to exclude (matched after stripping one
    /// component from the sort path). `*` matches any characters
    /// including `/`; `?` matches any single character.
    /// Corresponds to `excludes` in nixpkgs.
    pub excludes: Vec<String>,
    /// Glob patterns for files to include (matched after stripping one
    /// component from the sort path). If non-empty, only files matching
    /// at least one pattern are included.
    /// Corresponds to `includes` in nixpkgs.
    pub includes: Vec<String>,
    /// 1-based indices of hunks to keep per file. If empty, all hunks
    /// are kept. Corresponds to `hunks` in nixpkgs.
    pub hunks: Vec<usize>,
    /// When true, reverse the patch (swap old/new, insert/remove).
    /// Corresponds to `revert` in nixpkgs.
    pub revert: bool,
}

/// Download a patch from a URL, normalize it, and compute its flat SHA-256 hash.
///
/// This mimics the normalization done by nixpkgs' `fetchpatch`:
/// 1. Discard preamble (content before the first `---` line)
/// 2. Remove `diff --git` headers (contain variable commit SHAs)
/// 3. Remove `index` lines (contain variable object hashes)
/// 4. Remove binary file markers and `GIT binary patch` lines
/// 5. Remove function/context text from hunk headers (text after `@@ ... @@`)
/// 6. Remove empty hunks (hunks with only context lines, no additions/deletions)
/// 7. Sort file sections alphabetically by path
/// 8. Strip path components from all file paths
///
/// The hash is computed as `sha256(normalized_content)` using flat hashing
/// (matching `fetchurl`'s default `outputHashMode = "flat"`).
pub struct PatchHasher;

impl PatchHasher {
    pub fn hash_patch_url(url: &str, options: &PatchOptions) -> Result<NarHash> {
        let patch_bytes = Self::download(url)?;
        let content = std::str::from_utf8(&patch_bytes)
            .with_context(|| format!("Patch content from {url} is not valid UTF-8"))?;
        let normalized = normalize_patch(content, options)?;
        Ok(nix_prefetch_git::nar::flat_hash(normalized.as_bytes()))
    }

    fn download(url: &str) -> Result<Vec<u8>> {
        let response = ureq::get(url)
            .call()
            .with_context(|| format!("Failed to download patch from {url}"))?;
        let mut bytes = Vec::new();
        response
            .into_body()
            .into_reader()
            .read_to_end(&mut bytes)
            .with_context(|| format!("Failed to read patch content from {url}"))?;
        Ok(bytes)
    }
}

/// Normalize a patch content string.
///
/// The `options` parameter controls all normalization behavior including
/// path stripping, filtering, and reversal.
fn normalize_patch(content: &str, options: &PatchOptions) -> Result<String> {
    // Pre-process lines to handle "\\ No newline at end of file" markers.
    // patchkit's iter_file_patch doesn't include these markers in Patch
    // entries when they appear after the last counted hunk line, so we
    // pre-process them here: strip the trailing newline from the preceding
    // line and remove the marker. patchkit's HunkLine serialization will
    // re-add the marker for lines that lack a trailing newline.
    let lines: Vec<Vec<u8>> = {
        let mut result: Vec<Vec<u8>> = Vec::new();
        for line in content.split_inclusive('\n') {
            let line_bytes = line.as_bytes();
            if line_bytes.starts_with(b"\\ No newline at end of file") {
                if let Some(prev) = result.last_mut()
                    && prev.ends_with(b"\n")
                {
                    prev.pop();
                }
            } else {
                result.push(line_bytes.to_vec());
            }
        }
        result
    };

    // Parse with patchkit
    let patches: Vec<PlainOrBinaryPatch> = parse_patches(lines.into_iter())
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| "Failed to parse patch")?;

    // Compute effective strip length.
    // When `relative` is set, we need to strip one extra component (the
    // a/b prefix that filterdiff -p1 strips for matching) plus the
    // components in the relative path, in addition to strip_len.
    let effective_strip = if let Some(ref relative) = options.relative {
        1 + count_path_segments(relative) + options.strip_len
    } else {
        options.strip_len
    };

    // Process each patch
    let mut processed: Vec<(String, UnifiedPatch)> = Vec::new();

    for patch in patches {
        match patch {
            PlainOrBinaryPatch::Plain(mut unified) => {
                // Clean hunks: strip function/context text from hunk headers
                // (like `filterdiff --clean`)
                for hunk in &mut unified.hunks {
                    hunk.tail = None;
                }

                // Apply hunks selection before empty hunk removal so that
                // the 1-based indices correspond to the original hunks.
                if !options.hunks.is_empty() {
                    let selected: Vec<_> = options
                        .hunks
                        .iter()
                        .filter_map(|&i| {
                            if i > 0 {
                                unified.hunks.get(i - 1).cloned()
                            } else {
                                None
                            }
                        })
                        .collect();
                    unified.hunks = selected;
                }

                // Filter out empty hunks (no added or deleted lines, only context)
                unified.hunks.retain(|hunk| {
                    hunk.lines.iter().any(|line| {
                        matches!(line, HunkLine::InsertLine(_) | HunkLine::RemoveLine(_))
                    })
                });

                // Skip patches with no hunks remaining
                if unified.hunks.is_empty() {
                    continue;
                }

                // Determine sort path: use orig_name (the --- a/ path) for
                // modified and deleted files, and mod_name (the +++ b/ path)
                // for new files. This matches `lsdiff | LC_ALL=C sort -u`
                // behavior used by nixpkgs' filterdiff pipeline, where lsdiff
                // reports the --- a/ path for existing files and the +++ b/
                // path for new files. Using orig_name ensures all a/ paths
                // sort together (before b/ paths), matching nixpkgs' sort order.
                let sort_path = {
                    let orig_path = path_string(&unified.orig_name);
                    if orig_path == "/dev/null" {
                        // New file: use the +++ b/ path
                        path_string(&unified.mod_name)
                    } else {
                        // Modified or deleted file: use the --- a/ path
                        orig_path
                    }
                };

                // Apply relative filter: only include files whose path
                // (after stripping one component) starts with `${relative}/`.
                // This mimics `filterdiff -p1 -i '${relative}/*'`.
                if let Some(ref relative) = options.relative {
                    let match_path = strip_path_n(&sort_path, 1);
                    if !match_path.starts_with(&format!("{}/", relative)) {
                        continue;
                    }
                }

                // Strip path components from orig_name and mod_name
                unified.orig_name = strip_path_bytes(&unified.orig_name, effective_strip);
                unified.mod_name = strip_path_bytes(&unified.mod_name, effective_strip);

                // Apply extraPrefix: prepend a/${extra_prefix}/ to old paths
                // and b/${extra_prefix}/ to new paths. This mimics
                // `filterdiff --addoldprefix=a/${extraPrefix} --addnewprefix=b/${extraPrefix}`.
                // When `relative` is set, nixpkgs sets extraPrefix to "" (empty string)
                // if it was null, which causes --addoldprefix=a/ --addnewprefix=b/ to be
                // added. This effectively re-adds the a/b prefixes after stripping.
                // When both `relative` and `extraPrefix` are set, the prefix is
                // a/${extraPrefix}.
                let needs_prefix = options.relative.is_some() || options.extra_prefix.is_some();
                if needs_prefix {
                    let prefix = match &options.extra_prefix {
                        Some(ep) => ep.as_str(),
                        None => "", // relative is set but no extra_prefix: use empty string
                    };
                    unified.orig_name =
                        add_prefix_bytes(&unified.orig_name, &format!("a/{}", prefix));
                    unified.mod_name =
                        add_prefix_bytes(&unified.mod_name, &format!("b/{}", prefix));
                }

                processed.push((sort_path, unified));
            }
            PlainOrBinaryPatch::Binary(_) => {
                // Skip binary patches
            }
        }
    }

    // Apply excludes/includes filtering on the sort path with one
    // component stripped (matching `filterdiff -p1` semantics).
    processed.retain(|(sort_path, _)| {
        let match_path = strip_path_n(sort_path, 1);

        // If excludes is non-empty and the match path matches any exclude
        // pattern, exclude the file.
        if !options.excludes.is_empty()
            && options.excludes.iter().any(|pat| fnmatch(pat, &match_path))
        {
            return false;
        }

        // If includes is non-empty and the match path doesn't match any
        // include pattern, exclude the file.
        if !options.includes.is_empty()
            && !options.includes.iter().any(|pat| fnmatch(pat, &match_path))
        {
            return false;
        }

        true
    });

    // Sort by path alphabetically (matching `lsdiff | sort -u`)
    processed.sort_by(|a, b| a.0.cmp(&b.0));

    // Apply revert: reverse the patch by swapping old/new names,
    // positions, and line types. This mimics `interdiff $out /dev/null`.
    if options.revert {
        for (_, patch) in &mut processed {
            std::mem::swap(&mut patch.orig_name, &mut patch.mod_name);
            std::mem::swap(&mut patch.orig_ts, &mut patch.mod_ts);

            for hunk in &mut patch.hunks {
                std::mem::swap(&mut hunk.orig_pos, &mut hunk.mod_pos);
                std::mem::swap(&mut hunk.orig_range, &mut hunk.mod_range);
                for line in &mut hunk.lines {
                    *line = match std::mem::replace(line, HunkLine::ContextLine(Vec::new())) {
                        HunkLine::InsertLine(bytes) => HunkLine::RemoveLine(bytes),
                        HunkLine::RemoveLine(bytes) => HunkLine::InsertLine(bytes),
                        HunkLine::ContextLine(bytes) => HunkLine::ContextLine(bytes),
                    };
                }
            }
        }
    }

    // Re-serialize each patch using as_bytes(), concatenating them.
    // patchkit's as_bytes() only outputs --- / +++ lines and hunk content,
    // no diff --git, index, or other metadata — matching filterdiff behavior.
    // Do not add blank lines between sections — matching current behavior.
    let mut result = Vec::new();
    for (_, patch) in processed {
        if options.revert {
            result.extend_from_slice(b"reverted:\n");
        }
        result.extend_from_slice(&patch.as_bytes());
    }

    // Ensure the result ends with a newline (matching fetchpatch behavior)
    if !result.is_empty() && result.last() != Some(&b'\n') {
        result.push(b'\n');
    }

    String::from_utf8(result).with_context(|| "Normalized patch content is not valid UTF-8")
}

/// Count the number of path segments in a path, splitting by `/`.
///
/// E.g., "src" → 1, "src/sub" → 2.
fn count_path_segments(path: &str) -> usize {
    path.split('/').filter(|s| !s.is_empty()).count()
}

/// Prepend a prefix to a path in its byte representation, handling
/// C-style quoting properly. If the path is `/dev/null`, don't add
/// a prefix.
fn add_prefix_bytes(path: &[u8], prefix: &str) -> Vec<u8> {
    let s = String::from_utf8_lossy(path);

    // Note: we intentionally do NOT skip /dev/null here. nixpkgs' filterdiff
    // --addoldprefix/--addnewprefix applies the prefix to /dev/null paths as
    // well, producing paths like a/pkg/dev/null or a//dev/null. We must match
    // that behavior for hash compatibility.

    if s.starts_with('"') {
        // C-style quoted path: unquote, add prefix, re-quote if needed
        let (unquoted, _) = match ansi_c_unquote(&s) {
            Some(result) => result,
            None => {
                // If unquoting fails, just prepend literally
                let mut result = prefix.to_string().into_bytes();
                result.extend_from_slice(path);
                return result;
            }
        };

        let prefixed = format!("{}{}", prefix, unquoted);
        if needs_quoting(&prefixed) {
            c_quote(&prefixed).into_bytes()
        } else {
            prefixed.into_bytes()
        }
    } else {
        // Unquoted path
        let prefixed = format!("{}{}", prefix, s);
        if needs_quoting(&prefixed) {
            c_quote(&prefixed).into_bytes()
        } else {
            prefixed.into_bytes()
        }
    }
}

/// Extract the path string from raw bytes, handling C-style quoting.
///
/// If the path is C-style quoted (starts with `"`), it is unquoted.
/// Otherwise, it is converted from UTF-8 lossily.
fn path_string(path: &[u8]) -> String {
    let s = String::from_utf8_lossy(path);
    if s.starts_with('"') {
        ansi_c_unquote(&s)
            .map(|(unquoted, _)| unquoted)
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

/// Strip N path components from a byte path, handling C-style quoting.
///
/// If the path is C-style quoted (starts with `"`): unquote with
/// `ansi_c_unquote`, strip with `strip_path_n`, re-quote with `c_quote`
/// if needed.
///
/// If unquoted: convert to string, strip with `strip_path_n`, convert back.
///
/// Never strips `/dev/null`. Returns the path as-is if `n == 0`.
fn strip_path_bytes(path: &[u8], n: usize) -> Vec<u8> {
    if n == 0 {
        return path.to_vec();
    }

    let s = String::from_utf8_lossy(path);

    if s.starts_with('"') {
        // C-style quoted path
        let (unquoted, _) = match ansi_c_unquote(&s) {
            Some(result) => result,
            None => return path.to_vec(),
        };

        // Don't strip /dev/null
        if unquoted == "/dev/null" {
            return path.to_vec();
        }

        let stripped = strip_path_n(&unquoted, n);

        if needs_quoting(&stripped) {
            c_quote(&stripped).into_bytes()
        } else {
            stripped.into_bytes()
        }
    } else {
        // Unquoted path
        // Don't strip /dev/null
        if s == "/dev/null" {
            return path.to_vec();
        }

        let stripped = strip_path_n(&s, n);
        stripped.into_bytes()
    }
}

/// Strip N path components from a file path.
///
/// For example, `strip_path_n("a/path/to/file", 1)` returns "path/to/file",
/// removing the smallest prefix containing 1 leading slash.
fn strip_path_n(path: &str, n: usize) -> String {
    let mut result = path;
    for _ in 0..n {
        if let Some(idx) = result.find('/') {
            result = &result[idx + 1..];
        } else {
            // No more slashes to strip
            break;
        }
    }
    result.to_string()
}

/// Unquote a C-style (ANSI-C) quoted path using git's quoting rules,
/// delegating to `gix_quote::ansi_c::undo`.
///
/// If `input` starts with `"`, it is treated as a C-style quoted string
/// and the unquoted content plus the number of consumed bytes (including
/// the surrounding quotes) are returned.
///
/// If `input` does not start with `"`, it is returned unchanged with
/// consumed = `input.len()`. Returns `None` on malformed input or if
/// the quoted string is not properly terminated with a closing `"`.
fn ansi_c_unquote(input: &str) -> Option<(String, usize)> {
    let is_quoted = input.starts_with('"');
    let bstr_input = bstr::BStr::new(input);
    match gix_quote::ansi_c::undo(bstr_input) {
        Ok((unquoted, consumed)) => {
            // gix_quote::ansi_c::undo does not treat an unclosed quote as
            // an error — it consumes to the end of input. Check that a
            // quoted string was properly terminated (the last consumed byte
            // must be the closing `"`).
            if is_quoted && input.as_bytes().get(consumed - 1) != Some(&b'"') {
                return None;
            }
            let s = String::from_utf8_lossy(&unquoted).to_string();
            Some((s, consumed))
        }
        Err(_) => None,
    }
}

/// Check if a path needs C-style quoting in git diff output.
fn needs_quoting(path: &str) -> bool {
    path.contains(' ')
        || path.contains('\\')
        || path.contains('"')
        || path.contains('\n')
        || path.contains('\t')
}

/// Apply C-style quoting to a path for git diff output.
fn c_quote(path: &str) -> String {
    if !needs_quoting(path) {
        return path.to_string();
    }
    let mut result = String::from("\"");
    for c in path.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\u{07}' => result.push_str("\\a"),
            '\u{08}' => result.push_str("\\b"),
            '\u{0c}' => result.push_str("\\f"),
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}

fn fnmatch(pattern: &str, text: &str) -> bool {
    GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher().is_match(text))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_path_n() {
        assert_eq!(strip_path_n("a/src/main.rs", 1), "src/main.rs");
        assert_eq!(strip_path_n("a/src/main.rs", 2), "main.rs");
        assert_eq!(strip_path_n("a/src/main.rs", 3), "main.rs"); // No more slashes
        assert_eq!(strip_path_n("src/main.rs", 1), "main.rs");
        assert_eq!(strip_path_n("main.rs", 1), "main.rs"); // No slash to strip
        assert_eq!(strip_path_n("a/b/c/d", 2), "c/d");
    }

    #[test]
    fn test_strip_path_bytes() {
        // Unquoted path
        assert_eq!(strip_path_bytes(b"a/src/main.rs", 1), b"src/main.rs");
        assert_eq!(strip_path_bytes(b"a/src/main.rs", 2), b"main.rs");
        assert_eq!(strip_path_bytes(b"a/src/main.rs", 3), b"main.rs"); // No more slashes
        // /dev/null is never stripped
        assert_eq!(strip_path_bytes(b"/dev/null", 1), b"/dev/null");
        // Zero strip
        assert_eq!(strip_path_bytes(b"a/src/main.rs", 0), b"a/src/main.rs");
        // C-style quoted path with spaces
        assert_eq!(
            strip_path_bytes(b"\"a/path with spaces\"", 1),
            b"\"path with spaces\""
        );
        // C-style quoted /dev/null
        assert_eq!(strip_path_bytes(b"\"/dev/null\"", 1), b"\"/dev/null\"");
        // C-style quoted path that doesn't need quoting after stripping
        assert_eq!(strip_path_bytes(b"\"a/simple\"", 1), b"simple");
        // Mnemonic prefixes
        assert_eq!(strip_path_bytes(b"i/file.txt", 1), b"file.txt");
        assert_eq!(strip_path_bytes(b"w/file.txt", 1), b"file.txt");
        // Multi-character prefixes
        assert_eq!(strip_path_bytes(b"origin/file.txt", 1), b"file.txt");
        assert_eq!(strip_path_bytes(b"modified/file.txt", 1), b"file.txt");
    }

    #[test]
    fn test_count_path_segments() {
        assert_eq!(count_path_segments("src"), 1);
        assert_eq!(count_path_segments("src/sub"), 2);
        assert_eq!(count_path_segments("src/sub/deep"), 3);
        assert_eq!(count_path_segments(""), 0);
    }

    #[test]
    fn test_add_prefix_bytes() {
        // add_prefix_bytes concatenates prefix and path WITHOUT a slash.
        // Callers include the trailing slash in the prefix if needed
        // (e.g. "a/pkg/" to produce "a/pkg/file.txt").
        // Simple unquoted path, prefix with trailing slash
        assert_eq!(add_prefix_bytes(b"file.txt", "a/pkg/"), b"a/pkg/file.txt");
        // Simple unquoted path, prefix without trailing slash (raw concatenation)
        assert_eq!(add_prefix_bytes(b"file.txt", "a/pkg"), b"a/pkgfile.txt");
        // /dev/null is prefixed (matching filterdiff --addoldprefix behavior)
        assert_eq!(add_prefix_bytes(b"/dev/null", "a/pkg/"), b"a/pkg//dev/null");
        // C-style quoted /dev/null is also prefixed
        assert_eq!(
            add_prefix_bytes(b"\"/dev/null\"", "a/pkg/"),
            b"a/pkg//dev/null"
        );
        // C-style quoted path with spaces, prefix with trailing slash
        assert_eq!(
            add_prefix_bytes(b"\"path with spaces\"", "a/pkg/"),
            b"\"a/pkg/path with spaces\""
        );
        // C-style quoted path with spaces, prefix without trailing slash
        assert_eq!(
            add_prefix_bytes(b"\"path with spaces\"", "a/pkg"),
            b"\"a/pkgpath with spaces\""
        );
        // Path that doesn't need quoting after prefix
        assert_eq!(add_prefix_bytes(b"file.txt", "a/pkg/"), b"a/pkg/file.txt");
    }

    #[test]
    fn test_fnmatch() {
        // Exact match
        assert!(fnmatch("file.txt", "file.txt"));
        assert!(!fnmatch("file.txt", "other.txt"));

        // * matches any sequence including /
        assert!(fnmatch("*", "file.txt"));
        assert!(fnmatch("*", "src/file.txt"));
        assert!(fnmatch("*.txt", "file.txt"));
        assert!(fnmatch("*.txt", "src/file.txt"));
        assert!(!fnmatch("*.txt", "file.rs"));
        assert!(fnmatch("src/*", "src/main.rs"));
        assert!(fnmatch("src/*", "src/sub/main.rs"));

        // ? matches single character
        assert!(fnmatch("file?.txt", "file1.txt"));
        assert!(!fnmatch("file?.txt", "file.txt"));
        assert!(!fnmatch("file?.txt", "file12.txt"));

        // Combined
        assert!(fnmatch("src/*/test", "src/foo/test"));
        assert!(fnmatch("src/*/test", "src/foo/bar/test"));
        assert!(fnmatch("?", "a"));
        assert!(!fnmatch("?", "ab"));
        assert!(fnmatch("", ""));
        assert!(!fnmatch("", "a"));
        assert!(!fnmatch("a", ""));
    }

    #[test]
    fn test_normalize_patch_removes_index_lines() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc1234..def5678 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old line
+new line
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!normalized.contains("index abc1234..def5678"));
        assert!(!normalized.contains("diff --git"));
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
    }

    #[test]
    fn test_normalize_patch_sorts_files() {
        let patch = "\
diff --git a/zebra.txt b/zebra.txt
index 111..222 100644
--- a/zebra.txt
+++ b/zebra.txt
@@ -1 +1 @@
-old
+new
diff --git a/apple.txt b/apple.txt
index 333..444 100644
--- a/apple.txt
+++ b/apple.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let apple_pos = normalized.find("apple.txt").unwrap();
        let zebra_pos = normalized.rfind("zebra.txt").unwrap();
        assert!(
            apple_pos < zebra_pos,
            "apple should come before zebra in sorted output"
        );
    }

    #[test]
    fn test_normalize_patch_strips_paths() {
        let patch = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 line1
-old
+new
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- src/main.rs"));
        assert!(normalized.contains("+++ src/main.rs"));
        assert!(!normalized.contains("a/src/main.rs"));
        assert!(!normalized.contains("diff --git"));
    }

    #[test]
    fn test_normalize_patch_removes_binary_files() {
        let patch = "\
diff --git a/image.png b/image.png
Binary files /dev/null and b/image.png differ
diff --git a/text.txt b/text.txt
index abc..def 100644
--- a/text.txt
+++ b/text.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!normalized.contains("Binary files"));
        assert!(normalized.contains("text.txt"));
        // The binary file section should be removed entirely (no hunks)
        assert!(!normalized.contains("image.png"));
    }

    #[test]
    fn test_normalize_patch_removes_empty_hunks() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
 line2
 line3
";
        // This hunk has no added or deleted lines, so the section should be removed
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // The section should be empty (no hunks with changes)
        assert!(normalized.is_empty() || !normalized.contains("@@"));
    }

    #[test]
    fn test_normalize_patch_removes_preamble() {
        let patch = "\
From abc123 Mon Sep 17 00:00:00 2001
From: Author <author@example.com>
Date: Mon, 1 Jan 2024 00:00:00 +0000
Subject: [PATCH] Fix something

---
 file.txt | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)

diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old
+new
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!normalized.contains("From:"));
        assert!(!normalized.contains("Subject:"));
        assert!(!normalized.contains("Date:"));
        assert!(!normalized.contains("diff --git"));
        assert!(normalized.contains("--- file.txt"));
    }

    #[test]
    fn test_normalize_patch_preserves_no_newline_marker() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-old
\\ No newline at end of file
+new
\\ No newline at end of file
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("\\ No newline at end of file"));
    }

    #[test]
    fn test_normalize_patch_removes_new_file_mode() {
        let patch = "\
diff --git a/newfile.txt b/newfile.txt
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/newfile.txt
@@ -0,0 +1,3 @@
+line1
+line2
+line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // new file mode lines are removed by fetchpatch normalization
        assert!(!normalized.contains("new file mode"));
        assert!(!normalized.contains("index 0000000"));
        assert!(normalized.contains("--- /dev/null"));
        assert!(normalized.contains("+++ newfile.txt"));
    }

    #[test]
    fn test_normalize_patch_removes_deleted_file_mode() {
        let patch = "\
diff --git a/oldfile.txt b/oldfile.txt
deleted file mode 100644
index abc1234..0000000
--- a/oldfile.txt
+++ /dev/null
@@ -1,3 +0,0 @@
-line1
-line2
-line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // deleted file mode lines are removed by fetchpatch normalization
        assert!(!normalized.contains("deleted file mode"));
        assert!(!normalized.contains("index abc1234"));
        assert!(normalized.contains("--- oldfile.txt"));
        assert!(normalized.contains("+++ /dev/null"));
    }

    #[test]
    fn test_normalize_patch_git_binary_patch() {
        let patch = "\
diff --git a/binary.bin b/binary.bin
index abc..def 100644
GIT binary patch
literal 10
ZcmXp.FaR@n
--
diff --git a/text.txt b/text.txt
index abc..def 100644
--- a/text.txt
+++ b/text.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // The binary file section should be removed entirely
        assert!(!normalized.contains("GIT binary patch"));
        assert!(!normalized.contains("literal 10"));
        assert!(!normalized.contains("binary.bin"));
        // The text file section should remain
        assert!(normalized.contains("text.txt"));
    }

    #[test]
    fn test_normalize_patch_strip_zero() {
        let patch = "\
diff --git a/src/file.txt b/src/file.txt
index abc..def 100644
--- a/src/file.txt
+++ b/src/file.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 0,
                ..Default::default()
            },
        )
        .unwrap();
        // With strip=0, paths are not modified (matching nixpkgs fetchpatch
        // default behavior where filterdiff -p1 only affects matching, not
        // output paths)
        assert!(normalized.contains("--- a/src/file.txt"));
        assert!(normalized.contains("+++ b/src/file.txt"));
        assert!(!normalized.contains("diff --git"));
    }

    #[test]
    fn test_normalize_patch_strip_two() {
        let patch = "\
diff --git a/dir/src/file.txt b/dir/src/file.txt
index abc..def 100644
--- a/dir/src/file.txt
+++ b/dir/src/file.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- src/file.txt"));
        assert!(normalized.contains("+++ src/file.txt"));
    }

    #[test]
    fn test_normalize_patch_strip_three() {
        let patch = "\
diff --git a/dir/src/file.txt b/dir/src/file.txt
index abc..def 100644
--- a/dir/src/file.txt
+++ b/dir/src/file.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 3,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
    }

    #[test]
    fn test_normalize_patch_traditional_diff() {
        let patch = "\
--- a/oldfile.txt\t2024-01-01 00:00:00.000000000 +0000
+++ b/newfile.txt\t2024-01-02 00:00:00.000000000 +0000
@@ -1,3 +1,3 @@
 line1
-old
+new
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // Traditional diffs without diff --git headers are still handled
        assert!(normalized.contains("--- oldfile.txt"));
        assert!(normalized.contains("+++ newfile.txt"));
    }

    #[test]
    fn test_normalize_patch_multiple_hunks() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old1
+new1
 line3
@@ -10,3 +10,3 @@
 line10
-old2
+new2
 line12
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // Both hunks should be preserved
        assert!(normalized.contains("@@ -1,3 +1,3 @@"));
        assert!(normalized.contains("@@ -10,3 +10,3 @@"));
    }

    #[test]
    fn test_normalize_patch_mixed_changes_and_context() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,5 +1,5 @@
 context1
-old
+new
 context2
 context3
 context4
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // The hunk should be preserved (it has changes)
        assert!(normalized.contains("@@ -1,5 +1,5 @@"));
        assert!(normalized.contains("-old"));
        assert!(normalized.contains("+new"));
    }

    #[test]
    fn test_normalize_patch_strips_hunk_context() {
        let patch = "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@ some function context
 line1
-old
+new
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        // The function/context text should be stripped from hunk headers
        assert!(normalized.contains("@@ -1,3 +1,3 @@"));
        assert!(!normalized.contains("some function context"));
    }

    #[test]
    fn test_ansi_c_unquote() {
        // Unquoted input is returned as-is
        let (result, consumed) = ansi_c_unquote("simple path").unwrap();
        assert_eq!(result, "simple path");
        assert_eq!(consumed, "simple path".len());

        // Quoted path with spaces
        let (result, consumed) = ansi_c_unquote("\"path with spaces\"").unwrap();
        assert_eq!(result, "path with spaces");
        assert_eq!(consumed, "\"path with spaces\"".len());

        // Backslash escape inside quotes
        let (result, _) = ansi_c_unquote("\"path\\\\slash\"").unwrap();
        assert_eq!(result, "path\\slash");

        // Quote escape inside quotes
        let (result, _) = ansi_c_unquote("\"path\\\"quote\"").unwrap();
        assert_eq!(result, "path\"quote");

        // Newline escape inside quotes
        let (result, _) = ansi_c_unquote("\"path\\nnewline\"").unwrap();
        assert_eq!(result, "path\nnewline");

        // Tab escape inside quotes
        let (result, _) = ansi_c_unquote("\"path\\ttab\"").unwrap();
        assert_eq!(result, "path\ttab");

        // Octal escape (\141 = 'a') inside quotes
        let (result, _) = ansi_c_unquote("\"octal\\141\"").unwrap();
        assert_eq!(result, "octala");

        // Unclosed quote returns None (gix_quote doesn't treat this as
        // an error, so we validate that consumed bytes end with `"`)
        assert_eq!(ansi_c_unquote("\"unclosed"), None);
        // Incomplete escape at end of quoted string returns None
        assert_eq!(ansi_c_unquote("\"incomplete\\"), None);
    }

    #[test]
    fn test_needs_quoting() {
        assert!(needs_quoting("path with spaces"));
        assert!(needs_quoting("path\\with\\backslash"));
        assert!(needs_quoting("path\"with\"quotes"));
        assert!(needs_quoting("path\nwith\nnewlines"));
        assert!(needs_quoting("path\twith\ttabs"));
        assert!(!needs_quoting("simple/path.rs"));
    }

    #[test]
    fn test_c_quote() {
        // No quoting needed
        assert_eq!(c_quote("simple/path.rs"), "simple/path.rs");
        // Quoting needed for spaces
        assert_eq!(c_quote("path with spaces"), "\"path with spaces\"");
        // Round-trip: c_quote(ansi_c_unquote(x)) for a quoted path with spaces
        let (unquoted, _) = ansi_c_unquote("\"path with spaces\"").unwrap();
        assert_eq!(c_quote(&unquoted), "\"path with spaces\"");
    }

    #[test]
    fn test_normalize_patch_with_spaces_in_paths() {
        let patch = "\
diff --git \"a/path with spaces\" \"b/path with spaces\"
index abc..def 100644
--- \"a/path with spaces\"
+++ \"b/path with spaces\"
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- \"path with spaces\""));
        assert!(normalized.contains("+++ \"path with spaces\""));
        assert!(!normalized.contains("diff --git"));
        assert!(!normalized.contains("index abc..def"));
    }

    #[test]
    fn test_normalize_patch_mnemonic_prefixes() {
        let patch = "\
diff --git i/file.txt w/file.txt
index abc..def 100644
--- i/file.txt
+++ w/file.txt
@@ -1 +1 @@
-old
+new
";
        // strip=0: prefixes preserved (matching filterdiff --strip=0 --clean)
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- i/file.txt"));
        assert!(normalized.contains("+++ w/file.txt"));
        // strip=1: one component stripped, regardless of prefix
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
    }

    #[test]
    fn test_normalize_patch_multichar_prefix() {
        let patch = "\
diff --git origin/file.txt modified/file.txt
index abc..def 100644
--- origin/file.txt
+++ modified/file.txt
@@ -1 +1 @@
-old
+new
";
        // strip=0: prefixes preserved
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- origin/file.txt"));
        assert!(normalized.contains("+++ modified/file.txt"));
        // strip=1: first component stripped (origin/ and modified/)
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
    }

    #[test]
    fn test_normalize_patch_relative_filter() {
        let patch = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1 +1 @@
-old
+new
diff --git a/lib/util.rs b/lib/util.rs
index abc..def 100644
--- a/lib/util.rs
+++ b/lib/util.rs
@@ -1 +1 @@
-old
+new
";
        // With relative = "src", only src/ files should be included
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                relative: Some("src".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // effective_strip = 1 + 1 + 0 = 2, so a/src/main.rs → main.rs
        assert!(normalized.contains("--- a/main.rs"));
        assert!(normalized.contains("+++ b/main.rs"));
        assert!(!normalized.contains("lib"));
        assert!(!normalized.contains("util.rs"));
    }

    #[test]
    fn test_normalize_patch_relative_with_strip() {
        let patch = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1 +1 @@
-old
+new
";
        // With relative = "src" and strip_len = 1, effective_strip = 1 + 1 + 1 = 3
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                relative: Some("src".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // With 3 components stripped from a/src/main.rs → .rs (just "main.rs" -> strip 3 -> "rs" since "a/src/main.rs" strip 3 -> "rs")
        // Wait: a/src/main.rs → strip 1 → src/main.rs → strip 2 → main.rs → strip 3 → main.rs (no more slashes)
        // Actually strip_path_n("a/src/main.rs", 3) = strip 1: "src/main.rs", strip 2: "main.rs", strip 3: "main.rs" (no slash)
        // Hmm, let me trace: "a/src/main.rs" find '/' at 1 -> "src/main.rs", find '/' at 3 -> "main.rs", no more '/' -> "main.rs"
        // So strip 3 from "a/src/main.rs" = "main.rs". That's only 2 actual components stripped.
        // effective_strip = 3, but strip_path_n caps at available slashes
        assert!(normalized.contains("main.rs"));
        assert!(!normalized.contains("src/main.rs"));
    }

    #[test]
    fn test_normalize_patch_extra_prefix() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                extra_prefix: Some("pkg/".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // After strip=1: file.txt → prepend a/pkg/ and b/pkg/
        assert!(normalized.contains("--- a/pkg/file.txt"));
        assert!(normalized.contains("+++ b/pkg/file.txt"));
    }

    #[test]
    fn test_normalize_patch_extra_prefix_with_dev_null() {
        let patch = "\
diff --git a/newfile.txt b/newfile.txt
new file mode 100644
--- /dev/null
+++ b/newfile.txt
@@ -0,0 +1 @@
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                extra_prefix: Some("pkg/".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // /dev/null gets the prefix (matching filterdiff behavior)
        assert!(normalized.contains("--- a/pkg//dev/null"));
        assert!(normalized.contains("+++ b/pkg/newfile.txt"));
    }

    #[test]
    fn test_normalize_patch_excludes() {
        let patch = "\
diff --git a/keep.txt b/keep.txt
index abc..def 100644
--- a/keep.txt
+++ b/keep.txt
@@ -1 +1 @@
-old
+new
diff --git a/skip.rs b/skip.rs
index abc..def 100644
--- a/skip.rs
+++ b/skip.rs
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                excludes: vec!["*.rs".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("keep.txt"));
        assert!(!normalized.contains("skip.rs"));
    }

    #[test]
    fn test_normalize_patch_includes() {
        let patch = "\
diff --git a/keep.txt b/keep.txt
index abc..def 100644
--- a/keep.txt
+++ b/keep.txt
@@ -1 +1 @@
-old
+new
diff --git a/skip.rs b/skip.rs
index abc..def 100644
--- a/skip.rs
+++ b/skip.rs
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                includes: vec!["*.txt".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("keep.txt"));
        assert!(!normalized.contains("skip.rs"));
    }

    #[test]
    fn test_normalize_patch_hunks_selection() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old1
+new1
 line3
@@ -10,3 +10,3 @@
 line10
-old2
+new2
 line12
@@ -20,3 +20,3 @@
 line20
-old3
+new3
 line22
";
        // Keep only hunks 1 and 3 (1-based)
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                hunks: vec![1, 3],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("new1"));
        assert!(!normalized.contains("new2"));
        assert!(normalized.contains("new3"));
    }

    #[test]
    fn test_normalize_patch_revert() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old
+new
 line3
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                revert: true,
                ..Default::default()
            },
        )
        .unwrap();
        // After revert: old name and new name swapped, InsertLine ↔ RemoveLine
        // Each reversed section gets a "reverted:" prefix (mimicking interdiff).
        assert!(normalized.starts_with("reverted:\n"));
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
        assert!(normalized.contains("+old"));
        assert!(normalized.contains("-new"));
        assert!(!normalized.contains("-old"));
        assert!(!normalized.contains("+new"));
    }

    #[test]
    fn test_normalize_patch_revert_new_file() {
        let patch = "\
diff --git a/newfile.txt b/newfile.txt
new file mode 100644
--- /dev/null
+++ b/newfile.txt
@@ -0,0 +1,1 @@
+content
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                revert: true,
                ..Default::default()
            },
        )
        .unwrap();
        // After revert: the file should be deleted (swapped names)
        assert!(normalized.starts_with("reverted:\n"));
        assert!(normalized.contains("--- newfile.txt"));
        assert!(normalized.contains("+++ /dev/null"));
        assert!(normalized.contains("-content"));
        assert!(!normalized.contains("+content"));
    }

    #[test]
    fn test_normalize_patch_revert_deleted_file() {
        let patch = "\
diff --git a/oldfile.txt b/oldfile.txt
deleted file mode 100644
--- a/oldfile.txt
+++ /dev/null
@@ -1,1 +0,0 @@
-content
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                revert: true,
                ..Default::default()
            },
        )
        .unwrap();
        // After revert: the file should be created (swapped names)
        assert!(normalized.starts_with("reverted:\n"));
        assert!(normalized.contains("--- /dev/null"));
        assert!(normalized.contains("+++ oldfile.txt"));
        assert!(normalized.contains("+content"));
        assert!(!normalized.contains("-content"));
    }

    #[test]
    fn test_normalize_patch_relative_nested() {
        let patch = "\
diff --git a/src/sub/file.rs b/src/sub/file.rs
index abc..def 100644
--- a/src/sub/file.rs
+++ b/src/sub/file.rs
@@ -1 +1 @@
-old
+new
diff --git a/src/other.rs b/src/other.rs
index abc..def 100644
--- a/src/other.rs
+++ b/src/other.rs
@@ -1 +1 @@
-old
+new
";
        // relative = "src/sub" → only files under src/sub/ included
        // effective_strip = 1 + 2 + 0 = 3
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                relative: Some("src/sub".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // a/src/sub/file.rs → strip 3 → "file.rs" (strip 1: src/sub/file.rs, strip 2: sub/file.rs, strip 3: file.rs)
        assert!(normalized.contains("--- a/file.rs"));
        assert!(!normalized.contains("other.rs"));
    }

    #[test]
    fn test_normalize_patch_excludes_and_includes() {
        let patch = "\
diff --git a/keep.txt b/keep.txt
index abc..def 100644
--- a/keep.txt
+++ b/keep.txt
@@ -1 +1 @@
-old
+new
diff --git a/skip.rs b/skip.rs
index abc..def 100644
--- a/skip.rs
+++ b/skip.rs
@@ -1 +1 @@
-old
+new
diff --git a/also_keep.txt b/also_keep.txt
index abc..def 100644
--- a/also_keep.txt
+++ b/also_keep.txt
@@ -1 +1 @@
-old
+new
";
        // Exclude *.rs, include *.txt — excludes applied first
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                excludes: vec!["*.rs".to_string()],
                includes: vec!["*.txt".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.contains("keep.txt"));
        assert!(normalized.contains("also_keep.txt"));
        assert!(!normalized.contains("skip.rs"));
    }

    #[test]
    fn test_normalize_patch_hunks_empty_after_selection() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-old1
+new1
 line3
@@ -10,3 +10,3 @@
 line10
-old2
+new2
 line12
";
        // Select only hunk 5 (doesn't exist) → no hunks → file removed
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                hunks: vec![5],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.is_empty() || !normalized.contains("@@"));
    }

    #[test]
    fn test_normalize_patch_extra_prefix_with_spaces() {
        let patch = "\
diff --git \"a/path with spaces\" \"b/path with spaces\"
index abc..def 100644
--- \"a/path with spaces\"
+++ \"b/path with spaces\"
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 1,
                extra_prefix: Some("pkg/".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // After strip=1: "path with spaces" (C-quoted) → add prefix a/pkg/ and b/pkg/
        assert!(normalized.contains("--- \"a/pkg/path with spaces\""));
        assert!(normalized.contains("+++ \"b/pkg/path with spaces\""));
    }

    #[test]
    fn test_normalize_patch_revert_strip_zero() {
        let patch = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-old
+new
";
        let normalized = normalize_patch(
            patch,
            &PatchOptions {
                strip_len: 0,
                revert: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(normalized.starts_with("reverted:\n"));
        assert!(normalized.contains("--- b/file.txt"));
        assert!(normalized.contains("+++ a/file.txt"));
        assert!(normalized.contains("+old"));
        assert!(normalized.contains("-new"));
    }
}
