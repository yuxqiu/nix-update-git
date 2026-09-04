use std::path::PathBuf;

use crate::checker::FileResult;
use crate::rules::Update;

/// A diff-style view of the updates found in one file, independent of which
/// rule produced them. Rule identity is kept on each `Hunk` (for `--verbose`
/// output and tests) but is not part of the default rendering.
pub struct FileDiff {
    pub path: PathBuf,
    pub hunks: Vec<Hunk>,
}

/// One atomically-applied group of changes (mirrors `UpdateGroup`).
pub struct Hunk {
    pub rule_name: String,
    pub target: Option<String>,
    pub changes: Vec<LineChange>,
    pub updates: Vec<Update>,
}

/// One field's old/new value, resolved once from the source text.
pub struct LineChange {
    pub field: String,
    pub old: String,
    pub new: String,
}

/// Strips control characters (including the ESC byte that starts ANSI/CSI
/// escape sequences) from text that ultimately originates from the nix file
/// or an upstream git remote, before it reaches a terminal.
fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

impl FileDiff {
    pub fn from_result(fr: &FileResult) -> Self {
        let hunks = fr
            .updates_per_rule
            .iter()
            .flat_map(|(rule_name, groups)| {
                groups.iter().map(move |group| {
                    let target = group.updates.iter().find_map(|u| u.target.clone());
                    let changes = group
                        .updates
                        .iter()
                        .map(|u| LineChange {
                            field: u.field.clone(),
                            old: sanitize(&fr.content[u.range.start..u.range.end]),
                            new: sanitize(&u.replacement),
                        })
                        .collect();
                    Hunk {
                        rule_name: rule_name.clone(),
                        target: target.map(|t| sanitize(&t)),
                        changes,
                        updates: group.updates.clone(),
                    }
                })
            })
            .collect();

        FileDiff {
            path: fr.file_path.clone(),
            hunks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_strips_escape_and_control_chars() {
        assert_eq!(sanitize("v1.0\u{1b}[31mFAKE\u{1b}[0m"), "v1.0[31mFAKE[0m");
        assert_eq!(sanitize("plain-value"), "plain-value");
        assert_eq!(sanitize("has\ttab\nand\rcr"), "hastabandcr");
    }
}
