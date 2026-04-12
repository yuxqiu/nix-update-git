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
    let sha256 = extract_json_string(json, "sha256").unwrap_or_default();
    let hash = extract_json_string(json, "hash").unwrap_or_default();
    let rev = extract_json_string(json, "rev").unwrap_or_default();

    if sha256.is_empty() && hash.is_empty() {
        anyhow::bail!("nix-prefetch-git output missing sha256 and hash fields");
    }

    Ok(PrefetchResult {
        sha256_nix: sha256,
        sri_hash: hash,
        rev,
    })
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let colon_pos = after_key.find(':')?;
    let value_part = &after_key[colon_pos + 1..];
    let trimmed = value_part.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let value_start = 1;
    let rest = &trimmed[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"url": "https://example.com", "rev": "abc123", "sha256": "0abc", "hash": "sha256-xyz"}"#;
        assert_eq!(extract_json_string(json, "rev"), Some("abc123".to_string()));
        assert_eq!(
            extract_json_string(json, "sha256"),
            Some("0abc".to_string())
        );
        assert_eq!(
            extract_json_string(json, "hash"),
            Some("sha256-xyz".to_string())
        );
    }

    #[test]
    fn test_parse_prefetch_json() {
        let json = r#"{"url": "https://example.com", "rev": "abc123", "sha256": "0abc", "hash": "sha256-xyz"}"#;
        let result = parse_prefetch_json(json).unwrap();
        assert_eq!(result.sha256_nix, "0abc");
        assert_eq!(result.sri_hash, "sha256-xyz");
        assert_eq!(result.rev, "abc123");
    }
}
