use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use assert_cmd::Command;
use insta::glob;
use serde::Serialize;

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

#[test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
pub fn test_fetcher_snapshots() {
    insta::with_settings!({
        prepend_module_to_snapshot => false,
    }, {
        glob!("data/**/*.nix", |nix_path| {
            let redact_fields = parse_redact_directive(nix_path);

            let json_output = run_json_check(nix_path.to_str().unwrap());
            let entries = parse_json(&json_output, &redact_fields);

            let output_for_insta = serde_json::to_string_pretty(&entries).unwrap();

            let data_dir = Path::new("data");
            let nix_path_dir = nix_path.parent().unwrap();

            let relative = if let Some(pos) = nix_path_dir.components().position(|c| {
                c.as_os_str() == data_dir.as_os_str()
            }) {
                nix_path_dir.components().skip(pos).collect::<PathBuf>()
            } else {
                nix_path_dir.into()
            };
            let snap_dir = Path::new("snaps").join(relative.strip_prefix("data").unwrap());

            let snapshot_name = nix_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

            insta::with_settings!({
                prepend_module_to_snapshot => false,
                snapshot_path => snap_dir,
                snapshot_suffix => ""
            }, {
                insta::assert_snapshot!(snapshot_name, output_for_insta);
            });
        });
    });
}
