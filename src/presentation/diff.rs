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
                            old: fr.content[u.range.start..u.range.end].to_string(),
                            new: u.replacement.clone(),
                        })
                        .collect();
                    Hunk {
                        rule_name: rule_name.clone(),
                        target,
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
