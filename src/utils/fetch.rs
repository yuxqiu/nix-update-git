use anyhow::{Context, Result};
use std::process::Command;

use super::version::VersionDetector;

pub struct GitFetcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefType {
    Tags,
    Heads,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    Tag,
    Branch,
    Other,
}

#[derive(Debug, Clone)]
pub struct GitRef {
    pub sha: String,
    pub kind: RefKind,
    pub name: String,
    pub full_ref: String,
}

/// Builds `git ls-remote` args, terminated with `--` before the URL so a
/// URL/path starting with `-` (e.g. a malicious `git+file://-upload-pack=...`
/// flake input) can never be parsed as a flag by git.
fn build_ls_remote_args(ref_types: &[RefType], url: &str) -> Vec<String> {
    let mut args = vec!["ls-remote".to_string()];
    for rt in ref_types {
        match rt {
            RefType::Tags => {
                args.push("--tags".to_string());
                args.push("--refs".to_string());
            }
            RefType::Heads => {
                args.push("--heads".to_string());
            }
        }
    }
    args.push("--".to_string());
    args.push(url.to_string());
    args
}

impl GitFetcher {
    /// # Errors
    ///
    /// Returns an error if `git ls-remote` fails to execute or exits
    /// unsuccessfully.
    pub fn list_refs(url: &str, ref_types: &[RefType]) -> Result<Vec<GitRef>> {
        let args = build_ls_remote_args(ref_types, url);

        let output = Command::new("git")
            .args(&args)
            .output()
            .with_context(|| format!("Failed to execute git ls-remote for {}", url))?;

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
                let (kind, name) = full_ref.strip_prefix("refs/tags/").map_or_else(
                    || {
                        full_ref.strip_prefix("refs/heads/").map_or_else(
                            || (RefKind::Other, full_ref.clone()),
                            |name| (RefKind::Branch, name.to_string()),
                        )
                    },
                    |name| (RefKind::Tag, name.to_string()),
                );
                refs.push(GitRef {
                    sha: sha.to_string(),
                    kind,
                    name,
                    full_ref,
                });
            }
        }

        Ok(refs)
    }

    /// # Errors
    ///
    /// Returns an error if listing remote refs fails (see [`Self::list_refs`]).
    pub fn get_latest_tag(url: &str) -> Result<Option<String>> {
        Self::get_latest_tag_matching(url, None)
    }

    /// # Errors
    ///
    /// Returns an error if listing remote refs fails (see [`Self::list_refs`]).
    pub fn get_latest_tag_matching(url: &str, current: Option<&str>) -> Result<Option<String>> {
        let refs = Self::list_refs(url, &[RefType::Tags])?;
        let tags: Vec<&GitRef> = refs.iter().filter(|r| r.kind == RefKind::Tag).collect();

        if tags.is_empty() {
            return Ok(None);
        }

        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        let result = current.map_or_else(
            || VersionDetector::latest(&tag_names),
            |cur| VersionDetector::latest_matching(&tag_names, cur),
        );
        Ok(result.map(std::string::ToString::to_string))
    }

    /// # Errors
    ///
    /// Returns an error if listing remote refs fails (see [`Self::list_refs`]).
    pub fn get_latest_commit(url: &str, branch: &str) -> Result<Option<String>> {
        let refs = Self::list_refs(url, &[RefType::Heads])?;
        let branch_ref = refs
            .iter()
            .find(|r| r.kind == RefKind::Branch && r.name == branch);
        Ok(branch_ref.map(|r| r.sha.clone()))
    }

    /// # Errors
    ///
    /// Returns an error if listing remote refs fails (see [`Self::list_refs`]).
    pub fn resolve_ref_to_sha(url: &str, tag: &str) -> Result<Option<String>> {
        let refs = Self::list_refs(url, &[RefType::Tags])?;
        let tag_ref = refs
            .iter()
            .find(|r| r.kind == RefKind::Tag && r.name == tag);
        Ok(tag_ref.map(|r| r.sha.clone()))
    }

    /// # Errors
    ///
    /// Returns an error if listing remote refs fails (see [`Self::list_refs`]).
    pub fn list_tags(url: &str) -> Result<Vec<(String, String)>> {
        let refs = Self::list_refs(url, &[RefType::Tags])?;
        Ok(refs
            .into_iter()
            .filter(|r| r.kind == RefKind::Tag)
            .map(|r| (r.name, r.sha))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ls_remote_args_separates_url_from_flags() {
        let args = build_ls_remote_args(&[RefType::Tags], "--upload-pack=touch /tmp/pwned");
        assert_eq!(
            args,
            vec![
                "ls-remote",
                "--tags",
                "--refs",
                "--",
                "--upload-pack=touch /tmp/pwned",
            ]
        );
    }

    #[test]
    fn test_build_ls_remote_args_normal_url() {
        let args = build_ls_remote_args(&[RefType::Heads], "https://example.com/repo.git");
        assert_eq!(
            args,
            vec!["ls-remote", "--heads", "--", "https://example.com/repo.git"]
        );
    }
}
