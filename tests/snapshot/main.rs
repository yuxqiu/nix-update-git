use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use insta;
use libtest_mimic::{Arguments, Failed, Trial};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Serialize, Debug)]
pub struct SnapshotEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<[usize; 2]>,
}

fn parse_redact_directive(nix_path: &Path) -> HashSet<String> {
    let content = fs::read_to_string(nix_path).unwrap_or_default();
    let first_line = content.lines().next().unwrap_or("");
    if let Some(rest) = first_line.strip_prefix("# redact:") {
        rest.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        HashSet::new()
    }
}

fn run_json_check(file: &str) -> String {
    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--format").arg("json").arg(file);
    let output = cmd.output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn parse_json(json: &str, redact_fields: &HashSet<String>) -> Vec<SnapshotEntry> {
    let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    raw.into_iter()
        .map(|v| {
            let rule = v["rule"].as_str().unwrap_or("").to_string();
            let field = v["field"].as_str().unwrap_or("").to_string();
            let old = v["old"].as_str().unwrap_or("").to_string();
            let new = v["new"].as_str().unwrap_or("").to_string();
            let range = [
                v["range"][0].as_u64().unwrap_or(0) as usize,
                v["range"][1].as_u64().unwrap_or(0) as usize,
            ];

            SnapshotEntry {
                rule: if redact_fields.contains("rule") {
                    None
                } else {
                    Some(rule)
                },
                field: if redact_fields.contains("field") {
                    None
                } else {
                    Some(field)
                },
                old: if redact_fields.contains("old") {
                    None
                } else {
                    Some(old)
                },
                new: if redact_fields.contains("new") {
                    None
                } else {
                    Some(new)
                },
                range: if redact_fields.contains("range") {
                    None
                } else {
                    Some(range)
                },
            }
        })
        .collect()
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
    let json_output = run_json_check(nix_path.to_str().unwrap());
    let entries = parse_json(&json_output, &redact_fields);
    let output_for_insta = serde_json::to_string_pretty(&entries)
        .map_err(|e| Failed::from(format!("Failed to serialize snapshot: {e}")))?;

    let snap_dir = snapshot_dir_for(nix_path, data_dir);
    let snapshot_name = nix_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Compute input_file metadata relative to the workspace root,
    // matching what `insta::glob!` sets automatically.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input_file = nix_path
        .strip_prefix(manifest_dir)
        .unwrap_or(nix_path)
        .to_string_lossy()
        .into_owned();

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

            let data_dir_clone = data_dir.clone();
            Trial::test(name, move || run_snapshot_test(&path, &data_dir_clone))
                .with_ignored_flag(!is_network)
        })
        .collect();

    libtest_mimic::run(&args, tests).exit();
}
