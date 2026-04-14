use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::utils::{NarHash, TarballHasher};

use super::kind::FetcherKind;

pub fn compute_hash(
    kind: &FetcherKind,
    params: &HashMap<String, String>,
    rev: &str,
) -> Result<NarHash> {
    match kind {
        FetcherKind::FetchFromGitHub => {
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromGitHub")?;
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromGitHub")?;
            let github_base = params
                .get("githubBase")
                .map(|s| s.as_str())
                .unwrap_or("github.com");
            let url = format!(
                "https://{}/{}/{}/archive/{}.tar.gz",
                github_base, owner, repo, rev
            );
            TarballHasher::hash_tarball_url(&url)
        }
        FetcherKind::FetchFromGitLab => {
            let domain = params
                .get("domain")
                .map(|s| s.as_str())
                .unwrap_or("gitlab.com");
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromGitLab")?;
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromGitLab")?;
            let url = format!(
                "https://{}/{}/{}/-/archive/{}/{}-{}.tar.gz",
                domain, owner, repo, rev, repo, rev
            );
            TarballHasher::hash_tarball_url(&url)
        }
        FetcherKind::FetchFromCodeberg => {
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromCodeberg")?;
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromCodeberg")?;
            let url = format!(
                "https://codeberg.org/{}/{}/archive/{}.tar.gz",
                owner, repo, rev
            );
            TarballHasher::hash_tarball_url(&url)
        }
        _ => anyhow::bail!("Unsupported fetcher for tarball hashing"),
    }
}
