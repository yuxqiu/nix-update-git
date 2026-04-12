#[cfg(feature = "network-tests")]
mod test {
    use std::path::PathBuf;

    use assert_cmd::Command;
    use insta::glob;
    use serde::Serialize;

    #[derive(Serialize, Debug)]
    pub struct SnapshotEntry {
        pub rule: String,
        pub field: String,
        pub old: String,
    }

    fn run_json_check(file: &str) -> String {
        let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
        cmd.arg("--format").arg("json").arg(file);
        let output = cmd.output().unwrap();
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn parse_json(json: &str) -> Vec<SnapshotEntry> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
        raw.into_iter()
            .map(|v| SnapshotEntry {
                rule: v["rule"].as_str().unwrap_or("").to_string(),
                field: v["field"].as_str().unwrap_or("").to_string(),
                old: v["old"].as_str().unwrap_or("").to_string(),
            })
            .collect()
    }

    fn redaction(input: &str) -> String {
        // do not redact anything for now
        let redactable = [];
        let mut result = input.to_string();
        for pattern in redactable {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, "<REDACTED>").to_string();
            }
        }
        result
    }

    #[test]
    pub fn test_fetcher_snapshots() {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
        }, {
            glob!("data/**/*.nix", |nix_path| {   // note: start from "snapshot/data"
                let json_output = run_json_check(nix_path.to_str().unwrap());
                let entries = parse_json(&json_output);

                let output_for_insta = redaction(
                    &serde_json::to_string_pretty(&entries).unwrap()
                );

                // Compute the relative directory inside "data/"
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
}
