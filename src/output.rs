use std::io::{self, Write};
use std::path::Path;

use nix_update_git::rules::Update;
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

pub fn print_updates(fr: &FileResult) {
    for (rule_name, updates) in &fr.updates_per_rule {
        println!(
            "{}: Found {} update(s) from rule '{}':",
            fr.file_path.display(),
            updates.len(),
            rule_name,
        );
        for u in updates {
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

pub fn print_json(entries: &[UpdateEntry]) {
    match serde_json::to_string_pretty(entries) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error serializing JSON: {e}"),
    }
}

fn prompt_confirmation(update: &Update, old_text: &str) -> bool {
    print!(
        "  Update {}? ({} -> {}) [y/N] ",
        update.field, old_text, update.replacement
    );
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

pub fn select_interactive(fr: &FileResult, updates: &[Update]) -> Vec<Update> {
    updates
        .iter()
        .filter(|u| {
            let old_text = &fr.content[u.range.start..u.range.end];
            prompt_confirmation(u, old_text)
        })
        .cloned()
        .collect()
}
