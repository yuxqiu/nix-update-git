mod check;
mod output;
mod patch;

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use nix_update_git::cli::OutputFormat;
use nix_update_git::rules::{
    FetcherRule, FlakeInputRule, RuleRegistry, build_dune_package_rule,
    build_emscripten_package_rule, build_gem_rule, build_go_module_rule,
    build_haskell_package_rule, build_mix_package_rule, build_npm_package_rule,
    build_python_package_rule, build_rebar3_release_rule, build_rust_package_rule,
    mk_derivation_rule,
};
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
    let rules = &cli.rules;
    let rule_enabled = |name: &str| rules.iter().any(|r| r.is_enabled(name));

    if rule_enabled("flake") {
        registry.register(FlakeInputRule);
    }
    if rule_enabled("fetcher") {
        registry.register(FetcherRule);
    }
    if rule_enabled("mk-derivation") {
        registry.register(mk_derivation_rule());
    }
    if rule_enabled("build-rust-package") {
        registry.register(build_rust_package_rule());
    }
    if rule_enabled("build-go-module") {
        registry.register(build_go_module_rule());
    }
    if rule_enabled("build-python-package") {
        registry.register(build_python_package_rule());
    }
    if rule_enabled("build-dune-package") {
        registry.register(build_dune_package_rule());
    }
    if rule_enabled("build-npm-package") {
        registry.register(build_npm_package_rule());
    }
    if rule_enabled("build-mix-package") {
        registry.register(build_mix_package_rule());
    }
    if rule_enabled("build-rebar3-release") {
        registry.register(build_rebar3_release_rule());
    }
    if rule_enabled("build-gem") {
        registry.register(build_gem_rule());
    }
    if rule_enabled("build-haskell-package") {
        registry.register(build_haskell_package_rule());
    }
    if rule_enabled("build-emscripten-package") {
        registry.register(build_emscripten_package_rule());
    }

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
