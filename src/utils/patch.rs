use std::io::Read;

use anyhow::{Context, Result};
use nix_prefetch_git::NarHash;
use patchkit::unified::{HunkLine, PlainOrBinaryPatch, UnifiedPatch, parse_patches};

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
/// 8. Strip `total_strip` path components from all file paths
///
/// The hash is computed as `sha256(normalized_content)` using flat hashing
/// (matching `fetchurl`'s default `outputHashMode = "flat"`).
///
/// `total_strip` is the number of path components to strip from all file
/// paths. For the default `fetchpatch` (stripLen=0 with no additional
/// stripping), this is 0 because `filterdiff -p1` only affects matching,
/// not output paths.
pub struct PatchHasher;

impl PatchHasher {
    pub fn hash_patch_url(url: &str, total_strip: usize) -> Result<NarHash> {
        let patch_bytes = Self::download(url)?;
        let content = std::str::from_utf8(&patch_bytes)
            .with_context(|| format!("Patch content from {url} is not valid UTF-8"))?;
        let normalized = normalize_patch(content, total_strip)?;
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
/// `total_strip` is the number of path components to strip from all
/// file paths. For the default `fetchpatch` (stripLen=0 with
/// `filterdiff -p1` for matching only), this is 0.
fn normalize_patch(content: &str, total_strip: usize) -> Result<String> {
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

                // Strip path components from orig_name and mod_name
                unified.orig_name = strip_path_bytes(&unified.orig_name, total_strip);
                unified.mod_name = strip_path_bytes(&unified.mod_name, total_strip);

                processed.push((sort_path, unified));
            }
            PlainOrBinaryPatch::Binary(_) => {
                // Skip binary patches
            }
        }
    }

    // Sort by path alphabetically (matching `lsdiff | sort -u`)
    processed.sort_by(|a, b| a.0.cmp(&b.0));

    // Re-serialize each patch using as_bytes(), concatenating them.
    // patchkit's as_bytes() only outputs --- / +++ lines and hunk content,
    // no diff --git, index, or other metadata — matching filterdiff behavior.
    // Do not add blank lines between sections — matching current behavior.
    let mut result = Vec::new();
    for (_, patch) in processed {
        result.extend_from_slice(&patch.as_bytes());
    }

    // Ensure the result ends with a newline (matching fetchpatch behavior)
    if !result.is_empty() && result.last() != Some(&b'\n') {
        result.push(b'\n');
    }

    String::from_utf8(result).with_context(|| "Normalized patch content is not valid UTF-8")
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 0).unwrap();
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
        let normalized = normalize_patch(patch, 2).unwrap();
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
        let normalized = normalize_patch(patch, 3).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 0).unwrap();
        assert!(normalized.contains("--- i/file.txt"));
        assert!(normalized.contains("+++ w/file.txt"));
        // strip=1: one component stripped, regardless of prefix
        let normalized = normalize_patch(patch, 1).unwrap();
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
        let normalized = normalize_patch(patch, 0).unwrap();
        assert!(normalized.contains("--- origin/file.txt"));
        assert!(normalized.contains("+++ modified/file.txt"));
        // strip=1: first component stripped (origin/ and modified/)
        let normalized = normalize_patch(patch, 1).unwrap();
        assert!(normalized.contains("--- file.txt"));
        assert!(normalized.contains("+++ file.txt"));
    }
}
