mod interactive;
mod patch;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use nix_update_git::checker::{FileResult, check_file};
use nix_update_git::cli::ColorMode;
use nix_update_git::presentation::{FileDiff, render};
use nix_update_git::rules::{CheckWarning, Update, build_registry};
use rayon::prelude::*;
use walkdir::WalkDir;

/// Resolves `--color` against `NO_COLOR`/`CLICOLOR`/`CLICOLOR_FORCE`/CI/TTY
/// detection (via `anstream`), the same convention `git`/`ripgrep` use.
/// `Auto` defers entirely to that detection; `Always`/`Never` force it.
fn resolve_color(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Always => anstream::ColorChoice::Always.write_global(),
        ColorMode::Never => anstream::ColorChoice::Never.write_global(),
        ColorMode::Auto => {}
    }
    matches!(
        anstream::AutoStream::choice(&std::io::stdout()),
        anstream::ColorChoice::Always | anstream::ColorChoice::AlwaysAnsi
    )
}

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
                    .filter_map(std::result::Result::ok)
                    .filter(|e| e.file_type().is_file())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "nix"))
                    .map(walkdir::DirEntry::into_path),
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

/// Result of trying to apply one file's accepted updates.
enum ApplyOutcome {
    /// Nothing to do for this file (no accepted updates).
    NoUpdates,
    /// Written successfully.
    Applied,
    /// Write failed; the error is already reported to stderr.
    Failed,
}

/// Applies `updates` to `path` (if non-empty).
///
/// Deliberately silent on success: in `--update` mode the diff already
/// printed above (or the interactive prompt just answered) says exactly
/// what changed in this file, so a per-file "Applied N update(s)" line
/// would only repeat that — and, with one printed per file, made it hard to
/// tell where one file's report ended and the next began. Callers count
/// `Applied` outcomes and print a single summary at the end of the run
/// instead. Failure is still reported here, per-file: unlike a successful
/// write, it's new information the diff couldn't have shown.
fn apply_and_report(path: &Path, content: &str, updates: &[Update]) -> ApplyOutcome {
    if updates.is_empty() {
        return ApplyOutcome::NoUpdates;
    }
    let new_content = patch::apply_updates(content, updates, path);
    match fs::write(path, &new_content) {
        Ok(()) => ApplyOutcome::Applied,
        Err(e) => {
            eprintln!("Error writing {}: {}", path.display(), e);
            ApplyOutcome::Failed
        }
    }
}

/// `--update --interactive`: prompt hunk-by-hunk, then apply only the
/// accepted updates. Returns `(had_errors, files_updated)`.
fn run_interactive(file_results: &[FileResult], color: bool) -> (bool, usize) {
    let diffs: Vec<FileDiff> = file_results.iter().map(FileDiff::from_result).collect();
    let selections = interactive::select(&diffs, color);
    let mut had_errors = false;
    let mut files_updated = 0;
    for (fr, to_apply) in file_results.iter().zip(selections) {
        match apply_and_report(&fr.file_path, &fr.content, &to_apply) {
            ApplyOutcome::Applied => files_updated += 1,
            ApplyOutcome::Failed => had_errors = true,
            ApplyOutcome::NoUpdates => {}
        }
    }
    (had_errors, files_updated)
}

/// Default (non-interactive) mode: render each file's diff, and apply every
/// update when `--update` is set. Returns `(had_errors, files_updated)`.
fn run_default(
    file_results: &[FileResult],
    cli: &nix_update_git::cli::Cli,
    color: bool,
) -> (bool, usize) {
    let mut had_errors = false;
    let mut files_updated = 0;
    for fr in file_results {
        let diff = FileDiff::from_result(fr);
        match render(&diff, cli.verbose, color) {
            Some(text) => println!("{text}"),
            None if cli.verbose => println!("{}: No updates found", fr.file_path.display()),
            None => {}
        }

        if cli.update {
            let to_apply = fr.all_updates();
            match apply_and_report(&fr.file_path, &fr.content, &to_apply) {
                ApplyOutcome::Applied => files_updated += 1,
                ApplyOutcome::Failed => had_errors = true,
                ApplyOutcome::NoUpdates => {}
            }
        }
    }
    (had_errors, files_updated)
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

    let color = resolve_color(cli.color);
    let (run_had_errors, files_updated) = if cli.update && cli.interactive {
        run_interactive(&file_results, color)
    } else {
        run_default(&file_results, &cli, color)
    };
    had_errors |= run_had_errors;

    if files_updated > 0 {
        println!("Updated {files_updated} file(s).");
    }

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}
