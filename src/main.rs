mod check;
mod output;
mod patch;

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use nix_update_git::cli::OutputFormat;
use nix_update_git::rules::{FetcherRule, FlakeInputRule, RuleRegistry};
use rayon::prelude::*;
use walkdir::WalkDir;

use check::check_file;
use output::{UpdateEntry, print_json, print_updates, select_interactive};
use patch::apply_updates;

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

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.jobs)
        .build_global()?;

    let mut registry = RuleRegistry::new();
    registry.register(FlakeInputRule::new());
    registry.register(FetcherRule::new());

    let results: Vec<_> = files.par_iter().map(|p| check_file(p, &registry)).collect();

    let mut had_errors = false;
    let mut json_entries: Vec<UpdateEntry> = Vec::new();

    for result in results {
        let fr = match result {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                had_errors = true;
                continue;
            }
        };

        if fr.updates_per_rule.is_empty() {
            if cli.verbose && cli.format == OutputFormat::Text {
                println!("{}: No updates found", fr.file_path.display());
            }
            continue;
        }

        match cli.format {
            OutputFormat::Text => print_updates(&fr),
            OutputFormat::Json => {
                json_entries.extend(
                    fr.all_updates()
                        .iter()
                        .map(|u| UpdateEntry::from_update(&fr.file_path, &fr.content, u)),
                );
            }
        }

        if cli.update {
            let to_apply = if cli.interactive {
                select_interactive(&fr, &fr.all_updates())
            } else {
                fr.all_updates()
            };

            if !to_apply.is_empty() {
                let new_content = apply_updates(&fr.content, &to_apply, &fr.file_path);
                match fs::write(&fr.file_path, &new_content) {
                    Ok(()) => {
                        if cli.format == OutputFormat::Text {
                            println!(
                                "{}: Applied {} update(s)",
                                fr.file_path.display(),
                                to_apply.len()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error writing {}: {}", fr.file_path.display(), e);
                        had_errors = true;
                    }
                }
            }
        }
    }

    if cli.format == OutputFormat::Json {
        print_json(&json_entries);
    }

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}
