use std::path::Path;

use nix_update_git::rules::Update;

pub fn apply_updates(content: &str, updates: &[Update], file_path: &Path) -> String {
    let mut sorted_updates: Vec<&Update> = updates.iter().collect();
    sorted_updates.sort_by_key(|u| u.range.start);

    let mut valid_updates: Vec<&Update> = Vec::new();
    let mut had_overlaps = false;

    for update in &sorted_updates {
        if let Some(last) = valid_updates.last()
            && update.range.start < last.range.end
        {
            had_overlaps = true;
            let old1 = &content[last.range.start..last.range.end];
            let old2 = &content[update.range.start..update.range.end];
            eprintln!(
                "Warning: overlapping update ranges detected in {}:",
                file_path.display()
            );
            eprintln!(
                "  Rule '{}' update '{}' at bytes {}..{} ({} -> {}) overlaps with rule '{}' update '{}' at bytes {}..{} ({} -> {})",
                last.rule_name,
                last.field,
                last.range.start,
                last.range.end,
                old1,
                last.replacement,
                update.rule_name,
                update.field,
                update.range.start,
                update.range.end,
                old2,
                update.replacement,
            );
            valid_updates.pop();
            continue;
        }
        valid_updates.push(update);
    }

    let mut result = content.to_string();
    for update in valid_updates.into_iter().rev() {
        let start = update.range.start;
        let end = update.range.end;
        if start >= result.len() || end > result.len() || start >= end {
            eprintln!(
                "Warning: skipping update with invalid range {}..{} (file length {})",
                start,
                end,
                result.len()
            );
            continue;
        }
        result.replace_range(start..end, &update.replacement);
    }

    if had_overlaps {
        eprintln!(
            "Warning: some updates in {} were skipped due to overlapping ranges.",
            file_path.display()
        );
        eprintln!(
            r#"  This is likely a bug in nix-update-git (v{}-{}). Please report it at
  https://github.com/yuxqiu/nix-update-git/issues — include the version, the file(s) involved, and the warnings above."#,
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HASH")
        );
    }

    result
}
