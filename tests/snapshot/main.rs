use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use libtest_mimic::{Arguments, Failed, Trial};
use nix_update_git::checker::check_file;
use nix_update_git::presentation::{FileDiff, render};
use nix_update_git::rules::{build_registry, default_rule_ids};
use walkdir::WalkDir;

fn parse_redact_directive(nix_path: &Path) -> HashSet<String> {
    let content = fs::read_to_string(nix_path).unwrap_or_default();
    let first_line = content.lines().next().unwrap_or("");
    if let Some(rest) = first_line.strip_prefix("# redact:") {
        rest.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        HashSet::new()
    }
}

/// Check if the `.nix` file has a `# ignored` directive on its first line.
fn is_ignored(nix_path: &Path) -> bool {
    let content = fs::read_to_string(nix_path).unwrap_or_default();
    let first_line = content.lines().next().unwrap_or("");
    first_line.starts_with("# ignored")
}

/// Runs the checker in-process (the same rule set the CLI uses by default)
/// and renders the result as diff text, redacting non-deterministic fields
/// per the fixture's `# redact:` directive. `new` is the only field ever
/// redacted in practice; `range` is no longer part of the rendered text at
/// all, so a `# redact: ... range` directive is accepted but has nothing to
/// do — it's a leftover from the JSON-based format this replaced.
fn render_snapshot(nix_path: &Path, redact_fields: &HashSet<String>) -> Result<String, Failed> {
    let registry = build_registry(&default_rule_ids());
    let fr = check_file(nix_path, &registry)
        .map_err(|e| Failed::from(format!("check_file failed: {e}")))?;
    let mut diff = FileDiff::from_result(&fr);

    if redact_fields.contains("new") {
        for hunk in &mut diff.hunks {
            for change in &mut hunk.changes {
                change.new = "<redacted>".to_string();
            }
        }
    }

    Ok(render(&diff, true).unwrap_or_default())
}

/// Discover all `.nix` files under `data/`, sorted for deterministic order.
fn discover_nix_files(data_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(data_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "nix")
        })
        .map(|entry| entry.into_path())
        .collect();
    files.sort();
    files
}

/// Compute the snapshot directory for a given nix file.
///
/// Given `data/fetcher/foo.nix`, returns `snaps/fetcher/`.
fn snapshot_dir_for(nix_path: &Path, data_dir: &Path) -> PathBuf {
    let relative = nix_path.parent().unwrap_or(data_dir);
    let relative = relative.strip_prefix(data_dir).unwrap_or(relative);
    Path::new("snaps").join(relative)
}

/// Run a single snapshot test for one `.nix` file.
fn run_snapshot_test(nix_path: &Path, data_dir: &Path) -> Result<(), Failed> {
    let redact_fields = parse_redact_directive(nix_path);

    // Render using a path relative to the workspace root, not the absolute
    // path `discover_nix_files` walks with — otherwise the snapshot bakes in
    // a machine-specific absolute path. `cargo test` runs with the package
    // root as its working directory, so the relative path still resolves.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative_path = nix_path.strip_prefix(manifest_dir).unwrap_or(nix_path);

    let output_for_insta = render_snapshot(relative_path, &redact_fields)?;

    let snap_dir = snapshot_dir_for(nix_path, data_dir);
    let snapshot_name = nix_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Compute input_file metadata relative to the workspace root,
    // matching what `insta::glob!` sets automatically.
    let input_file = relative_path.to_string_lossy().into_owned();

    insta::with_settings!({
        prepend_module_to_snapshot => false,
        snapshot_path => snap_dir,
        snapshot_suffix => "",
        input_file => &input_file,
    }, {
        insta::assert_snapshot!(snapshot_name, output_for_insta);
    });

    Ok(())
}

fn main() {
    let args = Arguments::from_args();

    // Resolve data/ relative to this test file's directory.
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshot");
    let data_dir = base_dir.join("data");

    let nix_files = discover_nix_files(&data_dir);
    let is_network = cfg!(feature = "network-tests");

    let tests: Vec<Trial> = nix_files
        .into_iter()
        .map(|path| {
            // Test name: relative path from data/, without extension
            // e.g. "fetcher/arkenfox_user_js_hash"
            let name = path
                .strip_prefix(&data_dir)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .into_owned();

            let ignored = is_ignored(&path);
            let data_dir_clone = data_dir.clone();
            Trial::test(name, move || run_snapshot_test(&path, &data_dir_clone))
                .with_ignored_flag(!is_network || ignored)
        })
        .collect();

    libtest_mimic::run(&args, tests).exit();
}
