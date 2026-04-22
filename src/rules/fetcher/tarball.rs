use anyhow::{Context, Result};

use crate::parser::ParsedAttrs;
use crate::utils::{NarHash, TarballHasher};

use super::kind::FetcherKind;

fn build_tarball_url(kind: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
    match kind {
        FetcherKind::FetchFromGitHub
        | FetcherKind::FetchFromGitea
        | FetcherKind::FetchFromForgejo => {
            let owner = parsed
                .strings
                .get("owner")
                .with_context(|| format!("missing 'owner' parameter for {}", kind.name()))?;
            let repo = parsed
                .strings
                .get("repo")
                .with_context(|| format!("missing 'repo' parameter for {}", kind.name()))?;
            let base = match kind {
                FetcherKind::FetchFromGitHub => parsed
                    .strings
                    .get("githubBase")
                    .map(|s| s.as_str())
                    .unwrap_or("github.com"),
                FetcherKind::FetchFromGitea | FetcherKind::FetchFromForgejo => parsed
                    .strings
                    .get("domain")
                    .with_context(|| format!("missing 'domain' parameter for {}", kind.name()))?,
                _ => unreachable!(),
            };
            Ok(format!(
                "https://{}/{}/{}/archive/{}.tar.gz",
                base, owner, repo, rev
            ))
        }
        FetcherKind::FetchFromGitLab => {
            let domain = parsed
                .strings
                .get("domain")
                .map(|s| s.as_str())
                .unwrap_or("gitlab.com");
            let owner = parsed
                .strings
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromGitLab")?;
            let repo = parsed
                .strings
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromGitLab")?;
            Ok(format!(
                "https://{}/{}/{}/-/archive/{}/{}-{}.tar.gz",
                domain, owner, repo, rev, repo, rev
            ))
        }
        FetcherKind::FetchFromCodeberg => {
            let owner = parsed
                .strings
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromCodeberg")?;
            let repo = parsed
                .strings
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromCodeberg")?;
            Ok(format!(
                "https://codeberg.org/{}/{}/archive/{}.tar.gz",
                owner, repo, rev
            ))
        }
        FetcherKind::FetchFromSourcehut => {
            let owner = parsed
                .strings
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromSourcehut")?;
            let owner_with_tilde = if owner.starts_with('~') {
                owner.clone()
            } else {
                format!("~{}", owner)
            };
            let repo = parsed
                .strings
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromSourcehut")?;
            let domain = parsed
                .strings
                .get("domain")
                .map(|s| s.as_str())
                .unwrap_or("sr.ht");
            let vc = parsed
                .strings
                .get("vc")
                .map(|s| s.as_str())
                .unwrap_or("git");
            Ok(format!(
                "https://{}.{}/{}/{}/archive/{}.tar.gz",
                vc, domain, owner_with_tilde, repo, rev
            ))
        }
        FetcherKind::FetchFromBitbucket => {
            let owner = parsed
                .strings
                .get("owner")
                .with_context(|| "missing 'owner' parameter for fetchFromBitbucket")?;
            let repo = parsed
                .strings
                .get("repo")
                .with_context(|| "missing 'repo' parameter for fetchFromBitbucket")?;
            let rev_or_tag = if parsed.strings.contains_key("tag") {
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
            let base_url = parsed
                .strings
                .get("url")
                .with_context(|| "missing 'url' parameter for fetchFromGitiles")?;
            let rev_or_tag = if parsed.strings.contains_key("tag") {
                format!("refs/tags/{}", rev)
            } else {
                rev.to_string()
            };
            Ok(format!("{}/+archive/{}.tar.gz", base_url, rev_or_tag))
        }
        FetcherKind::FetchFromRepoOrCz => {
            let repo = parsed
                .strings
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

pub fn compute_hash(kind: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> Result<NarHash> {
    let url = build_tarball_url(kind, parsed, rev)?;
    TarballHasher::hash_tarball_url(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(entries: &[(&str, &str)]) -> ParsedAttrs {
        let mut p = ParsedAttrs::default();
        for (k, v) in entries {
            p.strings.insert(k.to_string(), v.to_string());
        }
        p
    }

    #[test]
    fn test_build_tarball_url_gitea() {
        let p = params(&[
            ("domain", "gitea.example"),
            ("owner", "alice"),
            ("repo", "proj"),
        ]);
        let url = build_tarball_url(&FetcherKind::FetchFromGitea, &p, "v1.2.3").unwrap();
        assert_eq!(
            url,
            "https://gitea.example/alice/proj/archive/v1.2.3.tar.gz"
        );
    }

    #[test]
    fn test_build_tarball_url_bitbucket_tag_is_refs_tags() {
        let mut p = ParsedAttrs::default();
        p.strings.insert("owner".to_string(), "o".to_string());
        p.strings.insert("repo".to_string(), "r".to_string());
        p.strings.insert("tag".to_string(), "v1.0.0".to_string());
        let url = build_tarball_url(&FetcherKind::FetchFromBitbucket, &p, "v2.0.0").unwrap();
        assert_eq!(url, "https://bitbucket.org/o/r/get/refs/tags/v2.0.0.tar.gz");
    }

    #[test]
    fn test_build_tarball_url_gitiles_tag_is_refs_tags() {
        let mut p = ParsedAttrs::default();
        p.strings
            .insert("url".to_string(), "https://g.example/repo".to_string());
        p.strings.insert("tag".to_string(), "v1.0.0".to_string());
        let url = build_tarball_url(&FetcherKind::FetchFromGitiles, &p, "v2.0.0").unwrap();
        assert_eq!(
            url,
            "https://g.example/repo/+archive/refs/tags/v2.0.0.tar.gz"
        );
    }
}
