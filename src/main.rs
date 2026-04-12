use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use nix_update_git::cli::OutputFormat;
use nix_update_git::parser::NixFile;
use nix_update_git::rules::{FetcherRule, FlakeInputRule, RuleRegistry, Update};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Serialize)]
struct UpdateEntry {
    file: String,
    rule: String,
    field: String,
    old: String,
    new: String,
    range: [usize; 2],
}

fn apply_updates(content: &str, updates: &[Update], file_path: &std::path::Path) -> String {
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

fn process_file(
    file_path: &std::path::Path,
    registry: &RuleRegistry,
    update: bool,
    interactive: bool,
    verbose: bool,
) -> Result<bool> {
    let content = fs::read_to_string(file_path)?;
    let mut had_errors = false;

    match NixFile::parse(&content) {
        Ok(nix_file) => {
            let root_node = nix_file.root_node();

            match registry.check_all(&root_node) {
                Ok(results) => {
                    if results.is_empty() {
                        if verbose {
                            println!("{}: No updates found", file_path.display());
                        }
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
                                let old_text = &content[update.range.start..update.range.end];
                                println!(
                                    "  - {}: {} -> {}",
                                    update.field, old_text, update.replacement
                                );
                                all_updates.push(update.clone());
                            }
                        }

                        if update {
                            let mut updates_to_apply = Vec::new();
                            if interactive {
                                for update in &all_updates {
                                    let old_text = &content[update.range.start..update.range.end];
                                    if prompt_confirmation(update, old_text) {
                                        updates_to_apply.push(update.clone());
                                    }
                                }
                            } else {
                                updates_to_apply = all_updates;
                            }

                            if !updates_to_apply.is_empty() {
                                let new_content =
                                    apply_updates(&content, &updates_to_apply, file_path);
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

fn process_file_json(
    file_path: &std::path::Path,
    registry: &RuleRegistry,
    update: bool,
) -> Result<Vec<UpdateEntry>> {
    let content = fs::read_to_string(file_path)?;
    let nix_file = NixFile::parse(&content)?;
    let root_node = nix_file.root_node();
    let results = registry.check_all(&root_node)?;

    let mut entries = Vec::new();
    for (_rule_name, updates) in &results {
        for u in updates {
            let old_text = &content[u.range.start..u.range.end];
            entries.push(UpdateEntry {
                file: file_path.to_string_lossy().to_string(),
                rule: u.rule_name.clone(),
                field: u.field.clone(),
                old: old_text.to_string(),
                new: u.replacement.clone(),
                range: [u.range.start, u.range.end],
            });
        }
    }

    if update && !entries.is_empty() {
        let all_updates: Vec<Update> = results
            .into_iter()
            .flat_map(|(_, updates)| updates)
            .collect();
        let new_content = apply_updates(&content, &all_updates, file_path);
        fs::write(file_path, &new_content)?;
    }

    Ok(entries)
}

fn expand_inputs(inputs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut result = Vec::new();

    for input in inputs {
        if input.is_file() {
            if input.extension().is_some_and(|ext| ext == "nix") {
                result.push(input);
            }
            continue;
        }

        if input.is_dir() {
            for entry in WalkDir::new(input)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "nix") {
                    result.push(path.to_path_buf());
                }
            }
        }
    }

    result
}

fn main() -> Result<()> {
    let cli = nix_update_git::cli::Cli::parse();

    if cli.files_or_directories.is_empty() {
        anyhow::bail!("No files specified. Use --help for usage information.");
    }

    if cli.check && cli.update {
        anyhow::bail!("--check and --update are mutually exclusive.");
    }

    let files = expand_inputs(cli.files_or_directories);

    if files.is_empty() {
        anyhow::bail!("No .nix files found in the provided inputs.");
    }

    let mut registry = RuleRegistry::new();
    registry.register(FlakeInputRule::new());
    registry.register(FetcherRule::new());

    if cli.format == OutputFormat::Json {
        let mut all_entries: Vec<UpdateEntry> = Vec::new();
        let mut had_errors = false;

        for file_path in &files {
            if !file_path.exists() {
                eprintln!("File not found: {}", file_path.display());
                had_errors = true;
                continue;
            }

            match process_file_json(file_path, &registry, cli.update) {
                Ok(entries) => all_entries.extend(entries),
                Err(e) => {
                    eprintln!("{}", e);
                    had_errors = true;
                }
            }
        }

        println!("{}", serde_json::to_string_pretty(&all_entries).unwrap());

        if had_errors {
            std::process::exit(1);
        }
        return Ok(());
    }

    let mut all_ok = true;
    for file_path in &files {
        if !file_path.exists() {
            eprintln!("File not found: {}", file_path.display());
            all_ok = false;
            continue;
        }

        let should_update = cli.update;
        let ok = process_file(
            file_path,
            &registry,
            should_update,
            cli.interactive,
            cli.verbose,
        )?;
        if !ok {
            all_ok = false;
        }
    }

    if !all_ok {
        std::process::exit(1);
    }

    Ok(())
}
