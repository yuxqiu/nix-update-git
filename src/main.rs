use anyhow::Result;
use clap::Parser;
use nix_update_git::parser::NixFile;
use nix_update_git::rules::{FlakeInputRule, RuleRegistry, Update};
use std::fs;
use std::io::{self, Write};

/// Apply updates to file content, using a greedy interval-scheduling approach
/// to detect and skip overlapping ranges.
///
/// Sorts updates by start position and walks through them in order. If an
/// update's range overlaps with the last accepted update's range, the current
/// update is skipped and a warning is printed. The last accepted update is
/// also removed when an overlap is detected, since two overlapping ranges
/// cannot both be applied correctly.
///
/// Tradeoffs vs. an O(n²) pairwise check:
/// - We only report overlaps between adjacent updates in the sorted order.
///   If update A overlaps both B and C but B and C don't overlap each other,
///   we report A↔B and skip both, then accept C (which is correct: C doesn't
///   overlap A since A is now excluded). A full pairwise check would also
///   report A↔C, but C can safely be applied since A was excluded.
/// - In the rare case where three updates have transitive overlaps
///   (A overlaps B, B overlaps C, A does not overlap C), the greedy approach
///   correctly excludes A and B (due to their overlap) and accepts C (since
///   the last accepted before C does not overlap it), which produces valid
///   output. A full pairwise check would exclude all three.
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

    match NixFile::parse(file_path, &content) {
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
