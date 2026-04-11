use anyhow::Result;
use clap::Parser;
use nix_update_git::parser::NixFile;
use nix_update_git::rules::{FlakeInputRule, RuleRegistry, Update};
use std::fs;
use std::io::{self, Write};

fn apply_updates(content: &str, updates: &[Update]) -> String {
    let mut result = content.to_string();
    let mut sorted_updates: Vec<&Update> = updates.iter().collect();
    sorted_updates.sort_by_key(|u| std::cmp::Reverse(u.range.start));

    for update in sorted_updates {
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
        let new_text = format!("\"{}\"", update.new_value);
        result.replace_range(start..end, &new_text);
    }
    result
}

fn prompt_confirmation(update: &Update) -> bool {
    print!(
        "  Update {}? ({} -> {}) [y/N] ",
        update.field, update.old_value, update.new_value
    );
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn process_file(
    file_path: &std::path::Path,
    registry: &RuleRegistry,
    update: bool,
    interactive: bool,
) -> Result<bool> {
    let content = fs::read_to_string(file_path)?;
    let mut had_errors = false;

    match NixFile::parse(file_path, &content) {
        Ok(nix_file) => {
            let root_node = nix_file.root_node();

            match registry.check_all(&root_node) {
                Ok(results) => {
                    if results.is_empty() {
                        println!("{}: No updates found", file_path.display());
                    } else {
                        let mut all_updates: Vec<Update> = Vec::new();
                        for (rule_name, updates) in &results {
                            println!(
                                "{}: Found {} update(s) from rule '{}':",
                                file_path.display(),
                                updates.len(),
                                rule_name
                            );
                            for update in updates {
                                println!(
                                    "  - {}: {} -> {}",
                                    update.field, update.old_value, update.new_value
                                );
                                all_updates.push(update.clone());
                            }
                        }

                        if update {
                            let mut updates_to_apply = Vec::new();
                            if interactive {
                                for update in &all_updates {
                                    if prompt_confirmation(update) {
                                        updates_to_apply.push(update.clone());
                                    }
                                }
                            } else {
                                updates_to_apply = all_updates;
                            }

                            if !updates_to_apply.is_empty() {
                                let new_content = apply_updates(&content, &updates_to_apply);
                                fs::write(file_path, &new_content)?;
                                println!(
                                    "{}: Applied {} update(s)",
                                    file_path.display(),
                                    updates_to_apply.len()
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error checking {}: {}", file_path.display(), e);
                    had_errors = true;
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to parse {}: {}", file_path.display(), e);
            had_errors = true;
        }
    }

    Ok(!had_errors)
}

fn main() -> Result<()> {
    let cli = nix_update_git::cli::Cli::parse();

    if cli.files.is_empty() {
        eprintln!("No files specified. Use --help for usage information.");
        std::process::exit(1);
    }

    if cli.check && cli.update {
        eprintln!("Error: --check and --update are mutually exclusive.");
        std::process::exit(1);
    }

    let mut registry = RuleRegistry::new();
    registry.register(FlakeInputRule::new());

    let mut all_ok = true;
    for file_path in &cli.files {
        if !file_path.exists() {
            eprintln!("File not found: {}", file_path.display());
            all_ok = false;
            continue;
        }

        let should_update = cli.update;
        let ok = process_file(file_path, &registry, should_update, cli.interactive)?;
        if !ok {
            all_ok = false;
        }
    }

    if !all_ok {
        std::process::exit(1);
    }

    Ok(())
}
