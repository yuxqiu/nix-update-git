use std::io::Read;

use anyhow::{Context, Result};
use nix_prefetch_git::NarHash;

/// Download a patch from a URL, normalize it, and compute its flat SHA-256 hash.
///
/// This mimics the normalization done by nixpkgs' `fetchpatch`:
/// 1. Discard preamble (content before the first `diff --git` line)
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

/// A single file's diff section within a patch.
#[derive(Debug, Clone)]
struct FileSection {
    /// The canonical file path (extracted from `diff --git a/... b/...` or
    /// `+++ b/...`).
    path: String,
    /// All lines in this section, including the `diff --git` header.
    lines: Vec<String>,
}

/// Normalize a patch content string.
///
/// `total_strip` is the number of path components to strip from all
/// file paths. For the default `fetchpatch` (stripLen=0 with
/// `filterdiff -p1` for matching only), this is 0.
fn normalize_patch(content: &str, total_strip: usize) -> Result<String> {
    let sections = parse_sections(content);
    let mut sections: Vec<FileSection> = sections
        .into_iter()
        .filter_map(|mut s| {
            clean_section(&mut s.lines);
            if has_hunk(&s.lines) { Some(s) } else { None }
        })
        .collect();

    // Sort sections alphabetically by path (matching `lsdiff | sort -u`)
    sections.sort_by(|a, b| a.path.cmp(&b.path));

    // Re-serialize with path stripping. Do not add blank lines between
    // sections — the nixpkgs fetchpatch normalization does not insert
    // them, and extra blank lines would change the hash.
    let mut result = String::new();
    let mut first = true;
    for section in sections.iter() {
        for line in &section.lines {
            let stripped = strip_path_components_in_line(line, total_strip);
            if !first {
                result.push('\n');
            }
            result.push_str(&stripped);
            first = false;
        }
    }
    // Ensure the result ends with a newline (matching fetchpatch behavior)
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

/// Parse patch content into file sections.
///
/// Each section starts with a `diff --git` line and includes all lines
/// until the next `diff --git` line or end of file.
///
/// Content before the first `diff --git` line (preamble, email headers
/// from git format-patch, etc.) is discarded, matching the behavior of
/// `filterdiff --include` which only includes content for listed files.
fn parse_sections(content: &str) -> Vec<FileSection> {
    let mut sections: Vec<FileSection> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut current_path: Option<String> = None;

    for line in content.lines() {
        if let Some(path) = parse_diff_git_path(line) {
            // Start of a new file section; flush the previous one
            if let Some(path) = current_path.take() {
                sections.push(FileSection {
                    path,
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_path = Some(path);
            current_lines.push(line.to_string());
        } else if current_path.is_some() {
            current_lines.push(line.to_string());
        }
        // Lines before the first `diff --git` are preamble and are discarded
    }

    // Flush the last section
    if let Some(path) = current_path {
        sections.push(FileSection {
            path,
            lines: current_lines,
        });
    }

    // Handle patches without `diff --git` headers (traditional unified diffs).
    // These are treated as a single file section.
    if sections.is_empty()
        && !content.trim().is_empty()
        && let Some(path) = extract_path_from_traditional_diff(content)
    {
        sections.push(FileSection {
            path,
            lines: content.lines().map(String::from).collect(),
        });
    }

    sections
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

/// Parse old and new paths from a `diff --git` line's content after the prefix.
///
/// Handles both unquoted format (`a/old b/new`) and quoted format
/// (`"a/old path" "b/new path"`). Returns `(old_path, new_path)` with
/// `a/` and `b/` prefixes still attached.
fn parse_git_diff_paths(rest: &str) -> Option<(String, String)> {
    let rest = rest.trim();
    if rest.starts_with('"') {
        // Quoted format: "a/old path" "b/new path"
        // Use gix-quote to parse ANSI-C quoted paths
        let (old_path, consumed) = ansi_c_unquote(rest)?;
        let remaining = rest.get(consumed..)?.trim();
        let (new_path, _) = ansi_c_unquote(remaining)?;
        Some((old_path, new_path))
    } else {
        // Unquoted format: <prefix>/old_path <prefix>/new_path
        // Git diff prefixes can be arbitrary strings set via
        // --src-prefix / --dst-prefix (e.g. "a/", "b/", "origin/",
        // "modified/", or even empty with --no-prefix).
        // Find the separator between old and new paths by looking for
        // a space followed by a non-space character — the space is the
        // delimiter between the two path tokens in unquoted mode.
        let sep = rest.find(' ')?;
        let old_path = &rest[..sep];
        let new_path = &rest[sep + 1..]; // skip the space
        Some((old_path.to_string(), new_path.to_string()))
    }
}

/// Extract the file path from a `diff --git <prefix>/old <prefix>/new` line.
///
/// Returns the **old path with prefix** (e.g. `a/src/file.rs`,
/// `origin/src/file.rs`), matching how `lsdiff` reports paths for
/// use in `sort -u` ordering within the nixpkgs fetchpatch pipeline.
fn parse_diff_git_path(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix("diff --git ")?;
    let (old_path, _new_path) = parse_git_diff_paths(rest)?;
    Some(old_path)
}

/// Extract the file path from a traditional unified diff (no `diff --git` header).
///
/// Returns the path **with its diff prefix** (e.g. `b/src/file.rs`),
/// matching the `lsdiff`-style sort key convention used by
/// `parse_diff_git_path`.
fn extract_path_from_traditional_diff(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("+++ ") {
            // Check for C-style quoted path
            let path = if rest.starts_with('"') {
                let (unquoted, _) = ansi_c_unquote(rest)?;
                unquoted
            } else {
                // Remove timestamp if present (tab-separated)
                rest.split('\t').next().unwrap_or(rest).to_string()
            };
            // Return path with prefix (matching lsdiff sort key convention)
            return Some(path);
        }
    }
    None
}

/// Clean a section's lines in place by removing:
/// - `diff --git` headers (contain variable commit SHAs)
/// - `index` lines (git diff header index lines that contain variable hashes)
/// - `new file mode` and `deleted file mode` lines (contain file mode bits)
/// - Binary file markers and `GIT binary patch` lines
/// - Function/context text from hunk headers (the text after `@@ ... @@`)
/// - Empty hunks (hunks with only context lines, no additions or deletions)
fn clean_section(lines: &mut Vec<String>) {
    // Remove diff --git headers, index lines, file mode lines, and
    // binary markers. The nixpkgs fetchpatch normalization strips these
    // because they contain variable content (index hashes, git commit
    // SHAs, file mode bits) that would make the hash non-deterministic.
    lines.retain(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with("diff --git ")
            && !trimmed.starts_with("index ")
            && !trimmed.starts_with("new file mode ")
            && !trimmed.starts_with("deleted file mode ")
            && !trimmed.starts_with("Binary files ")
            && !trimmed.starts_with("GIT binary patch")
    });

    // Strip function/context text from hunk headers.
    // `filterdiff --clean` removes the text after `@@ -l,s +l,s @@`,
    // e.g. `@@ -52,7 +52,7 @@ some context` becomes `@@ -52,7 +52,7 @@`.
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@@") {
            // Find the closing @@ of the hunk header.
            // Format: @@ -l,s +l,s @@ optional context
            // Skip the opening "@@" (2 chars) and find the next "@@".
            if let Some(pos) = rest.find("@@") {
                let end_pos = 2 + pos + 2; // position after the closing "@@"
                *line = trimmed[..end_pos].to_string();
            }
        }
    }

    // Remove empty hunks
    remove_empty_hunks(lines);
}

/// Remove hunks that have no added or deleted lines (only context lines).
fn remove_empty_hunks(lines: &mut Vec<String>) {
    let mut result = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("@@") {
            // Start of a hunk; collect all lines until the next hunk or section end
            let hunk_start = result.len();
            result.push(lines[i].clone());
            i += 1;
            let mut has_changes = false;
            while i < lines.len() {
                let l = lines[i].trim();
                if l.starts_with("@@") || l.starts_with("diff --git ") {
                    break;
                }
                if l.starts_with('+') || l.starts_with('-') {
                    has_changes = true;
                }
                result.push(lines[i].clone());
                i += 1;
            }
            if !has_changes {
                // Remove the empty hunk
                result.truncate(hunk_start);
            }
        } else {
            result.push(lines[i].clone());
            i += 1;
        }
    }
    *lines = result;
}

/// Check if a section has at least one hunk.
fn has_hunk(lines: &[String]) -> bool {
    lines.iter().any(|line| line.trim().starts_with("@@"))
}

/// Strip path components in a single line.
///
/// This handles:
/// - `--- a/path` lines (with optional tab-separated timestamp)
/// - `+++ b/path` lines (with optional tab-separated timestamp)
/// - `--- /dev/null` and `+++ /dev/null` lines (not stripped)
fn strip_path_components_in_line(line: &str, n: usize) -> String {
    if n == 0 {
        return line.to_string();
    }

    let trimmed = line.trim();

    if let Some(rest) = trimmed.strip_prefix("diff --git ") {
        // Format: "diff --git a/old_path b/new_path" or
        //         "diff --git "a/path with spaces" "b/path with spaces""
        let (old_path, new_path) = match parse_git_diff_paths(rest) {
            Some(paths) => paths,
            None => return line.to_string(),
        };
        let stripped_old = strip_path_n(&old_path, n);
        let stripped_new = strip_path_n(&new_path, n);
        if needs_quoting(&stripped_old) || needs_quoting(&stripped_new) {
            format!(
                "diff --git {} {}",
                c_quote(&stripped_old),
                c_quote(&stripped_new)
            )
        } else {
            format!("diff --git {} {}", stripped_old, stripped_new)
        }
    } else if trimmed.starts_with("--- ") || trimmed.starts_with("+++ ") {
        let prefix_len = 4; // "--- " or "+++ "
        let (prefix, path_and_rest) = trimmed.split_at(prefix_len);
        // Check for C-style quoted path
        if path_and_rest.starts_with('"') {
            let (path, consumed) = match ansi_c_unquote(path_and_rest) {
                Some(result) => result,
                None => return line.to_string(),
            };
            // Don't strip /dev/null
            if path == "/dev/null" {
                return line.to_string();
            }
            let stripped = strip_path_n(&path, n);
            // Reconstruct with original prefix, any text after the quoted path, and re-quoting if needed
            let after_quote = path_and_rest.get(consumed..).unwrap_or("");
            if needs_quoting(&stripped) {
                format!("{}{}{}", prefix, c_quote(&stripped), after_quote)
            } else {
                format!("{}{}{}", prefix, stripped, after_quote)
            }
        } else {
            // Unquoted path: split on tab to separate path from timestamp
            let path = path_and_rest.split('\t').next().unwrap_or(path_and_rest);
            // Don't strip /dev/null
            if path == "/dev/null" {
                return line.to_string();
            }
            let stripped = strip_path_n(path, n);
            // Reconstruct with original prefix and any timestamp
            if let Some(tab_idx) = path_and_rest.find('\t') {
                let timestamp = &path_and_rest[tab_idx..];
                format!("{}{}{}", prefix, stripped, timestamp)
            } else {
                format!("{}{}", prefix, stripped)
            }
        }
    } else {
        line.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_git_path() {
        // Returns old path with prefix (lsdiff convention for sort key)
        assert_eq!(
            parse_diff_git_path("diff --git a/src/main.rs b/src/main.rs"),
            Some("a/src/main.rs".to_string())
        );
        assert_eq!(
            parse_diff_git_path("diff --git a/README.md b/README.md"),
            Some("a/README.md".to_string())
        );
        assert_eq!(parse_diff_git_path("some other line"), None);
        assert_eq!(parse_diff_git_path(""), None);
    }

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
    fn test_strip_path_components_in_line() {
        assert_eq!(
            strip_path_components_in_line("diff --git a/src/main.rs b/src/main.rs", 1),
            "diff --git src/main.rs src/main.rs"
        );
        assert_eq!(
            strip_path_components_in_line("--- a/src/main.rs", 1),
            "--- src/main.rs"
        );
        assert_eq!(
            strip_path_components_in_line("+++ b/src/main.rs", 1),
            "+++ src/main.rs"
        );
        // /dev/null should not be stripped
        assert_eq!(
            strip_path_components_in_line("--- /dev/null", 1),
            "--- /dev/null"
        );
        assert_eq!(
            strip_path_components_in_line("+++ /dev/null", 1),
            "+++ /dev/null"
        );
        // Timestamps should be preserved
        assert_eq!(
            strip_path_components_in_line(
                "--- a/src/main.rs\t2024-01-01 00:00:00.000000000 +0000",
                1
            ),
            "--- src/main.rs\t2024-01-01 00:00:00.000000000 +0000"
        );
        // Zero strip should be identity
        assert_eq!(
            strip_path_components_in_line("diff --git a/file b/file", 0),
            "diff --git a/file b/file"
        );
        // Non-path lines should be unchanged
        assert_eq!(
            strip_path_components_in_line("@@ -1,3 +1,3 @@", 1),
            "@@ -1,3 +1,3 @@"
        );
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
    fn test_parse_git_diff_paths() {
        // Unquoted paths with default a/b prefixes
        assert_eq!(
            parse_git_diff_paths("a/src/main.rs b/src/main.rs"),
            Some(("a/src/main.rs".to_string(), "b/src/main.rs".to_string()))
        );
        // Unquoted paths with mnemonic prefixes (diff.mnemonicprefix)
        assert_eq!(
            parse_git_diff_paths("i/src/main.rs w/src/main.rs"),
            Some(("i/src/main.rs".to_string(), "w/src/main.rs".to_string()))
        );
        // Committed vs worktree mnemonic prefixes
        assert_eq!(
            parse_git_diff_paths("c/src/main.rs w/src/main.rs"),
            Some(("c/src/main.rs".to_string(), "w/src/main.rs".to_string()))
        );
        // Origin vs side1 merge prefixes
        assert_eq!(
            parse_git_diff_paths("o/src/main.rs 1/src/main.rs"),
            Some(("o/src/main.rs".to_string(), "1/src/main.rs".to_string()))
        );
        // Multi-character custom prefixes (--src-prefix/--dst-prefix)
        assert_eq!(
            parse_git_diff_paths("origin/src/main.rs modified/src/main.rs"),
            Some((
                "origin/src/main.rs".to_string(),
                "modified/src/main.rs".to_string()
            ))
        );
        // Quoted paths with spaces
        assert_eq!(
            parse_git_diff_paths("\"a/path with spaces\" \"b/path with spaces\""),
            Some((
                "a/path with spaces".to_string(),
                "b/path with spaces".to_string()
            ))
        );
        // Quoted paths with escape sequences (\\ -> \)
        assert_eq!(
            parse_git_diff_paths("\"a/path\\\\quote\" \"b/path\\\\quote\""),
            Some(("a/path\\quote".to_string(), "b/path\\quote".to_string()))
        );
    }

    #[test]
    fn test_parse_diff_git_path_with_spaces() {
        // Standard a/b prefixes — returns old path with prefix (lsdiff convention)
        assert_eq!(
            parse_diff_git_path("diff --git a/src/main.rs b/src/main.rs"),
            Some("a/src/main.rs".to_string())
        );
        // Mnemonic prefixes (i/w for index vs worktree)
        assert_eq!(
            parse_diff_git_path("diff --git i/src/main.rs w/src/main.rs"),
            Some("i/src/main.rs".to_string())
        );
        // Committed vs worktree mnemonic prefixes (c/w)
        assert_eq!(
            parse_diff_git_path("diff --git c/src/main.rs w/src/main.rs"),
            Some("c/src/main.rs".to_string())
        );
        // Multi-character custom prefixes (--src-prefix/--dst-prefix)
        assert_eq!(
            parse_diff_git_path("diff --git origin/src/main.rs modified/src/main.rs"),
            Some("origin/src/main.rs".to_string())
        );
        // Quoted path with spaces
        assert_eq!(
            parse_diff_git_path("diff --git \"a/path with spaces\" \"b/path with spaces\""),
            Some("a/path with spaces".to_string())
        );
        // No match for non-diff lines
        assert_eq!(parse_diff_git_path("some other line"), None);
    }

    #[test]
    fn test_strip_path_components_in_line_quoted() {
        // Quoted path in diff --git header
        assert_eq!(
            strip_path_components_in_line(
                "diff --git \"a/path with spaces\" \"b/path with spaces\"",
                1
            ),
            "diff --git \"path with spaces\" \"path with spaces\""
        );
        // Unquoted path (no spaces) remains unquoted
        assert_eq!(
            strip_path_components_in_line("diff --git a/src/main.rs b/src/main.rs", 1),
            "diff --git src/main.rs src/main.rs"
        );
        // Quoted path in --- line
        assert_eq!(
            strip_path_components_in_line("--- \"a/path with spaces\"", 1),
            "--- \"path with spaces\""
        );
        // Quoted path in +++ line
        assert_eq!(
            strip_path_components_in_line("+++ \"b/path with spaces\"", 1),
            "+++ \"path with spaces\""
        );
        // Quoted path with timestamp in --- line
        assert_eq!(
            strip_path_components_in_line("--- \"a/path with spaces\"\t2024-01-01", 1),
            "--- \"path with spaces\"\t2024-01-01"
        );
        // Quoted /dev/null should not be stripped
        assert_eq!(
            strip_path_components_in_line("--- \"/dev/null\"", 1),
            "--- \"/dev/null\""
        );
    }

    #[test]
    fn test_extract_path_from_traditional_diff_quoted() {
        // Unquoted +++ line — returns path with prefix (lsdiff convention)
        assert_eq!(
            extract_path_from_traditional_diff("+++ b/src/main.rs\n"),
            Some("b/src/main.rs".to_string())
        );
        // Quoted +++ line with spaces
        assert_eq!(
            extract_path_from_traditional_diff("+++ \"b/path with spaces\"\n"),
            Some("b/path with spaces".to_string())
        );
        // Quoted +++ line with timestamp
        assert_eq!(
            extract_path_from_traditional_diff("+++ \"b/path with spaces\"\t2024-01-01\n"),
            Some("b/path with spaces".to_string())
        );
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
