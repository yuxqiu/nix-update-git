use anyhow::{Context, Result};
use std::process::Command;

use super::version::VersionDetector;

pub struct GitFetcher;

impl GitFetcher {
    pub fn list_refs(url: &str) -> Result<Vec<GitRef>> {
        let output = Command::new("git")
            .args(["ls-remote", "--tags", "--refs", url])
            .output()
            .context("Failed to execute git ls-remote")?;

        if !output.status.success() {
            anyhow::bail!(
                "git ls-remote failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut refs = Vec::new();

        for line in output_str.lines() {
            if let Some((sha, full_ref)) = line.split_once('\t') {
                let full_ref = full_ref.to_string();
                let (kind, name) = if full_ref.starts_with("refs/tags/") {
                    (
                        "tag",
                        full_ref.strip_prefix("refs/tags/").unwrap().to_string(),
                    )
                } else if full_ref.starts_with("refs/heads/") {
                    (
                        "branch",
                        full_ref.strip_prefix("refs/heads/").unwrap().to_string(),
                    )
                } else {
                    ("other", full_ref.clone())
                };
                refs.push(GitRef {
                    sha: sha.to_string(),
                    kind: kind.to_string(),
                    name: name.to_string(),
                    full_ref,
                });
            }
        }

        Ok(refs)
    }

    pub fn get_latest_tag(url: &str) -> Result<Option<String>> {
        let refs = Self::list_refs(url)?;
        let tags: Vec<&GitRef> = refs.iter().filter(|r| r.kind == "tag").collect();

        if tags.is_empty() {
            return Ok(None);
        }

        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        Ok(VersionDetector::latest(&tag_names).map(|s| s.to_string()))
    }

    pub fn get_sha_for_ref(url: &str, ref_name: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["ls-remote", url, ref_name])
            .output()
            .context("Failed to execute git ls-remote")?;

        if !output.status.success() {
            anyhow::bail!(
                "git ls-remote failed for ref {}: {}",
                ref_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = output_str.lines().next() {
            if let Some((sha, _)) = line.split_once('\t') {
                Ok(sha.to_string())
            } else {
                anyhow::bail!("Unexpected git ls-remote output format")
            }
        } else {
            anyhow::bail!("No ref found for {}", ref_name)
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitRef {
    pub sha: String,
    pub kind: String,
    pub name: String,
    pub full_ref: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_git_fetcher_creation() {
        let _fetcher = super::GitFetcher;
    }
}
