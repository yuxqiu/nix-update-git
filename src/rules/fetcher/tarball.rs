use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::utils::{NarHash, TarballHasher};

use super::kind::FetcherKind;

// TODO: Check whether the following url works for all platforms and
// see if symbolic refs (tags) need to be resolved to commit SHAs before
// constructing tarball URLs.
fn build_tarball_url(
    kind: &FetcherKind,
    params: &HashMap<String, String>,
    rev: &str,
) -> Result<String> {
    match kind {
        FetcherKind::FetchFromGitHub
        | FetcherKind::FetchFromGitea
        | FetcherKind::FetchFromForgejo => {
            let owner = params
                .get("owner")
                .with_context(|| format!("missing 'owner' parameter for {}", kind.name()))?;
            let repo = params
                .get("repo")
                .with_context(|| format!("missing 'repo' parameter for {}", kind.name()))?;
            let base = match kind {
                FetcherKind::FetchFromGitHub => params
                    .get("githubBase")
                    .map(|s| s.as_str())
                    .unwrap_or("github.com"),
                FetcherKind::FetchFromGitea | FetcherKind::FetchFromForgejo => params
                    .get("domain")
                    .map(|s| s.as_str())
                    .with_context(|| format!("missing 'domain' parameter for {}", kind.name()))?,
                _ => unreachable!(),
            };
            Ok(format!(
                "https://{}/{}/{}/archive/{}.tar.gz",
                base, owner, repo, rev
            ))
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
            Ok(format!(
                "https://{}/{}/{}/-/archive/{}/{}-{}.tar.gz",
                domain, owner, repo, rev, repo, rev
            ))
        }
        FetcherKind::FetchFromCodeberg => {
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromCodeberg")?;
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromCodeberg")?;
            Ok(format!(
                "https://codeberg.org/{}/{}/archive/{}.tar.gz",
                owner, repo, rev
            ))
        }
        FetcherKind::FetchFromSourcehut => {
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromSourcehut")?;
            let owner_with_tilde = if owner.starts_with('~') {
                owner.clone()
            } else {
                // Sourcehut archive endpoints use "~owner/repo" paths.
                // Align with `FetcherKind::git_url`.
                format!("~{}", owner)
            };
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromSourcehut")?;
            let domain = params.get("domain").map(|s| s.as_str()).unwrap_or("sr.ht");
            let vc = params.get("vc").map(|s| s.as_str()).unwrap_or("git");
            Ok(format!(
                "https://{}.{}/{}/{}/archive/{}.tar.gz",
                vc, domain, owner_with_tilde, repo, rev
            ))
        }
        FetcherKind::FetchFromBitbucket => {
            let owner = params
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromBitbucket")?;
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromBitbucket")?;
            let rev_or_tag = if params.contains_key("tag") {
                format!("refs/tags/{}", rev)
            } else {
                rev.to_string()
            };
            Ok(format!(
                "https://bitbucket.org/{}/{}/get/{}.tar.gz",
                owner, repo, rev_or_tag
            ))
        }
        FetcherKind::FetchFromGitiles => {
            let base_url = params
                .get("url")
                .with_context(|| "missing 'url' parameter for fetchFromGitiles")?;
            let rev_or_tag = if params.contains_key("tag") {
                format!("refs/tags/{}", rev)
            } else {
                rev.to_string()
            };
            Ok(format!("{}/+archive/{}.tar.gz", base_url, rev_or_tag))
        }
        FetcherKind::FetchFromSavannah => {
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromSavannah")?;
            let repo_tail = repo.rsplit('/').next().unwrap_or(repo);
            Ok(format!(
                "https://cgit.git.savannah.gnu.org/cgit/{}.git/snapshot/{}-{}.tar.gz",
                repo, repo_tail, rev
            ))
        }
        FetcherKind::FetchFromRepoOrCz => {
            let repo = params
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromRepoOrCz")?;
            Ok(format!(
                "https://repo.or.cz/{}.git/snapshot/{}.tar.gz",
                repo, rev
            ))
        }
        _ => anyhow::bail!("Unsupported fetcher for tarball hashing"),
    }
}

pub fn compute_hash(
    kind: &FetcherKind,
    params: &HashMap<String, String>,
    rev: &str,
) -> Result<NarHash> {
    let url = build_tarball_url(kind, params, rev)?;
    TarballHasher::hash_tarball_url(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_build_tarball_url_gitea() {
        let params = map(&[
            ("domain", "gitea.example"),
            ("owner", "alice"),
            ("repo", "proj"),
        ]);
        let url = build_tarball_url(&FetcherKind::FetchFromGitea, &params, "v1.2.3").unwrap();
        assert_eq!(
            url,
            "https://gitea.example/alice/proj/archive/v1.2.3.tar.gz"
        );
    }

    #[test]
    fn test_build_tarball_url_bitbucket_tag_is_refs_tags() {
        let params = map(&[("owner", "o"), ("repo", "r"), ("tag", "v1.0.0")]);
        let url = build_tarball_url(&FetcherKind::FetchFromBitbucket, &params, "v2.0.0").unwrap();
        assert_eq!(url, "https://bitbucket.org/o/r/get/refs/tags/v2.0.0.tar.gz");
    }

    #[test]
    fn test_build_tarball_url_gitiles_tag_is_refs_tags() {
        let params = map(&[("url", "https://g.example/repo"), ("tag", "v1.0.0")]);
        let url = build_tarball_url(&FetcherKind::FetchFromGitiles, &params, "v2.0.0").unwrap();
        assert_eq!(
            url,
            "https://g.example/repo/+archive/refs/tags/v2.0.0.tar.gz"
        );
    }
}
