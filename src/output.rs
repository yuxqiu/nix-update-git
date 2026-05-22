use std::io::{self, Write};
use std::path::Path;

use nix_update_git::rules::{CheckWarning, Update};
use serde::Serialize;

use crate::check::FileResult;

#[derive(Serialize)]
pub struct UpdateEntry {
    file: String,
    rule: String,
    field: String,
    old: String,
    new: String,
    range: [usize; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
}

impl UpdateEntry {
    pub fn from_update(file: &Path, content: &str, u: &Update) -> Self {
        let old_text = &content[u.range.start..u.range.end];
        Self {
            file: file.to_string_lossy().to_string(),
            rule: u.rule_name.clone(),
            field: u.field.clone(),
            old: old_text.to_string(),
            new: u.replacement.clone(),
            range: [u.range.start, u.range.end],
            target: u.target.clone(),
        }
    }
}

pub fn print_warnings(warnings: &[CheckWarning]) {
    for warning in warnings {
        eprintln!("Warning: {warning}");
    }
}

pub fn print_updates(fr: &FileResult) {
    for (rule_name, groups) in &fr.updates_per_rule {
        let total_updates: usize = groups.iter().map(|g| g.updates.len()).sum();
        println!(
            "{}: Found {} update(s) from rule '{}':",
            fr.file_path.display(),
            total_updates,
            rule_name,
        );
        for group in groups {
            for u in &group.updates {
                let old_text = &fr.content[u.range.start..u.range.end];
                if let Some(target) = u.target.as_deref() {
                    println!(
                        "  - {} ({}): {} -> {}",
                        u.field, target, old_text, u.replacement
                    );
                } else {
                    println!("  - {}: {} -> {}", u.field, old_text, u.replacement);
                }
            }
        }
    }
}

pub fn print_json(entries: &[UpdateEntry]) {
    match serde_json::to_string_pretty(entries) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error serializing JSON: {e}"),
    }
}

fn prompt_group_confirmation(fr: &FileResult, group: &nix_update_git::rules::UpdateGroup) -> bool {
    // If group has only one update, prompt normally
    if group.updates.len() == 1 {
        let u = &group.updates[0];
        let old_text = &fr.content[u.range.start..u.range.end];
        print!(
            "  Update {}? ({} -> {}) [y/N] ",
            u.field, old_text, u.replacement
        );
    } else {
        // For multi-update groups, show all updates and prompt once
        println!("  Update group ({} changes):", group.updates.len());
        for u in &group.updates {
            let old_text = &fr.content[u.range.start..u.range.end];
            println!("    - {}: {} -> {}", u.field, old_text, u.replacement);
        }
        print!("  Apply all {}? [y/N] ", group.updates.len());
    }
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

pub fn select_interactive(
    fr: &FileResult,
    groups: &[&nix_update_git::rules::UpdateGroup],
) -> Vec<Update> {
    groups
        .iter()
        .filter(|group| prompt_group_confirmation(fr, group))
        .flat_map(|group| group.updates.clone())
        .collect()
}
