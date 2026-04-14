use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug)]
pub struct PrefetchResult {
    pub sha256_nix: String,
    pub sri_hash: String,
    pub rev: String,
}

pub struct NixPrefetcher;

impl NixPrefetcher {
    fn prefetch_git_inner(
        url: &str,
        rev: &str,
        extra_args: &[&str],
        label: &str,
    ) -> Result<PrefetchResult> {
        let mut args = vec!["--url", url, "--rev", rev, "--quiet"];
        args.extend(extra_args);

        let output = Command::new("nix-prefetch-git")
            .args(&args)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute nix-prefetch-git {}for {} @ {}",
                    label, url, rev
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "nix-prefetch-git {}failed for {} @ {}: {}{}",
                label,
                url,
                rev,
                stderr.trim(),
                if stdout.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", stdout.trim())
                }
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_prefetch_json(&stdout)
    }

    pub fn prefetch_git(url: &str, rev: &str) -> Result<PrefetchResult> {
        Self::prefetch_git_inner(url, rev, &[], "")
    }

    pub fn prefetch_git_with_submodules(url: &str, rev: &str) -> Result<PrefetchResult> {
        Self::prefetch_git_inner(url, rev, &["--fetch-submodules"], "(with submodules) ")
    }

    pub fn prefetch_archive(url: &str) -> Result<String> {
        let output = Command::new("nix-prefetch-url")
            .args(["--unpack", url])
            .output()
            .with_context(|| format!("Failed to execute nix-prefetch-url for {}", url))?;

        if !output.status.success() {
            anyhow::bail!(
                "nix-prefetch-url --unpack failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(hash)
    }

    pub fn is_available() -> bool {
        Command::new("nix-prefetch-git")
            .arg("--version")
            .output()
            .is_ok()
    }
}

fn parse_prefetch_json(json: &str) -> Result<PrefetchResult> {
    let value: serde_json::Value = serde_json::from_str(json)
        .with_context(|| "Failed to parse nix-prefetch-git JSON output")?;

    let sha256 = value
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let hash = value
        .get("hash")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let rev = value
        .get("rev")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if sha256.is_empty() && hash.is_empty() {
        anyhow::bail!("nix-prefetch-git output missing sha256 and hash fields");
    }

    Ok(PrefetchResult {
        sha256_nix: sha256,
        sri_hash: hash,
        rev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prefetch_json() {
        let json = r#"{"url": "https://example.com", "rev": "abc123", "sha256": "0abc", "hash": "sha256-xyz"}"#;
        let result = parse_prefetch_json(json).unwrap();
        assert_eq!(result.sha256_nix, "0abc");
        assert_eq!(result.sri_hash, "sha256-xyz");
        assert_eq!(result.rev, "abc123");
    }

    #[test]
    fn test_parse_prefetch_json_with_escaped_quotes() {
        let json = r#"{"url": "https://example.com/repo", "rev": "abc123", "sha256": "0abc", "hash": "sha256-xyz"}"#;
        let result = parse_prefetch_json(json).unwrap();
        assert_eq!(result.sha256_nix, "0abc");
        assert_eq!(result.sri_hash, "sha256-xyz");
    }

    #[test]
    fn test_parse_prefetch_json_missing_fields() {
        let json = r#"{"url": "https://example.com"}"#;
        let result = parse_prefetch_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_prefetch_json_invalid_json() {
        let result = parse_prefetch_json("not json");
        assert!(result.is_err());
    }
}
