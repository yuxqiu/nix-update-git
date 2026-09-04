mod interactive;
mod patch;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use nix_update_git::checker::{FileResult, check_file};
use nix_update_git::presentation::{FileDiff, render};
use nix_update_git::rules::{CheckWarning, Update, build_registry};
use rayon::prelude::*;
use walkdir::WalkDir;

fn expand_inputs(inputs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut result = Vec::new();

    for input in inputs {
        if input.is_file() {
            if input.extension().is_some_and(|ext| ext == "nix") {
                result.push(input);
            }
        } else if input.is_dir() {
            result.extend(
                WalkDir::new(input)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "nix"))
                    .map(|e| e.into_path()),
            );
        }
    }

    result
}

fn print_warnings(warnings: &[CheckWarning]) {
    for warning in warnings {
        eprintln!("Warning: {warning}");
    }
}

/// Applies `updates` to `path` (if non-empty) and reports the result.
/// Returns `false` if writing the file failed.
fn apply_and_report(path: &Path, content: &str, updates: &[Update]) -> bool {
    if updates.is_empty() {
        return true;
    }
    let new_content = patch::apply_updates(content, updates, path);
    match fs::write(path, &new_content) {
        Ok(()) => {
            println!("{}: Applied {} update(s)", path.display(), updates.len());
            true
        }
        Err(e) => {
            eprintln!("Error writing {}: {}", path.display(), e);
            false
        }
    }
}

/// `--update --interactive`: prompt hunk-by-hunk, then apply only the
/// accepted updates. Returns `true` if any file failed to write.
fn run_interactive(file_results: &[FileResult]) -> bool {
    let diffs: Vec<FileDiff> = file_results.iter().map(FileDiff::from_result).collect();
    let selections = interactive::select(&diffs);
    let mut had_errors = false;
    for (fr, to_apply) in file_results.iter().zip(selections) {
        if !apply_and_report(&fr.file_path, &fr.content, &to_apply) {
            had_errors = true;
        }
    }
    had_errors
}

/// Default (non-interactive) mode: render each file's diff, and apply every
/// update when `--update` is set. Returns `true` if any file failed to
/// write.
fn run_default(file_results: &[FileResult], cli: &nix_update_git::cli::Cli) -> bool {
    let mut had_errors = false;
    for fr in file_results {
        let diff = FileDiff::from_result(fr);
        match render(&diff, cli.verbose) {
            Some(text) => println!("{text}"),
            None if cli.verbose => println!("{}: No updates found", fr.file_path.display()),
            None => {}
        }

        if cli.update {
            let to_apply = fr.all_updates();
            if !apply_and_report(&fr.file_path, &fr.content, &to_apply) {
                had_errors = true;
            }
        }
    }
    had_errors
}

fn main() -> Result<()> {
    let cli = nix_update_git::cli::Cli::parse();

    if cli.files_or_directories.is_empty() {
        anyhow::bail!("No files specified. Use --help for usage information.");
    }

    if cli.check && cli.update {
        anyhow::bail!("--check and --update are mutually exclusive.");
    }

    let files = expand_inputs(cli.files_or_directories.clone());

    if files.is_empty() {
        anyhow::bail!("No .nix files found in the provided inputs.");
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.jobs)
        .build_global()?;

    let registry = build_registry(&cli.rules);

    let results: Vec<_> = files.par_iter().map(|p| check_file(p, &registry)).collect();

    let mut had_errors = false;
    let mut file_results: Vec<FileResult> = Vec::new();

    for result in results {
        match result {
            Ok(fr) => file_results.push(fr),
            Err(e) => {
                eprintln!("{e}");
                had_errors = true;
            }
        }
    }

    for fr in &file_results {
        if !fr.warnings.is_empty() {
            print_warnings(&fr.warnings);
        }
    }

    let run_had_errors = if cli.update && cli.interactive {
        run_interactive(&file_results)
    } else {
        run_default(&file_results, &cli)
    };
    had_errors |= run_had_errors;

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}
