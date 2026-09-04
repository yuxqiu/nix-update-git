use anyhow::Result;

use crate::parser::ParsedAttrs;
use crate::utils::{NarHash, TarballHasher};

use super::kind::FetcherKind;

pub fn compute_hash(kind: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> Result<NarHash> {
    let url = match kind {
        FetcherKind::Forge(forge) => forge.archive_url(parsed, rev)?,
        _ => anyhow::bail!("Unsupported fetcher for tarball hashing"),
    };
    TarballHasher::hash_tarball_url(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(fn_name: &str) -> FetcherKind {
        FetcherKind::from_name(fn_name).unwrap()
    }

    fn params(entries: &[(&str, &str)]) -> ParsedAttrs {
        let mut p = ParsedAttrs::default();
        for (k, v) in entries {
            p.strings.insert(k.to_string(), v.to_string());
        }
        p
    }

    fn archive_url(k: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> String {
        match k {
            FetcherKind::Forge(forge) => forge.archive_url(parsed, rev).unwrap(),
            _ => panic!("not a forge"),
        }
    }

    #[test]
    fn test_build_tarball_url_gitea() {
        let p = params(&[
            ("domain", "gitea.example"),
            ("owner", "alice"),
            ("repo", "proj"),
        ]);
        let url = archive_url(&kind("fetchFromGitea"), &p, "v1.2.3");
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
        let url = archive_url(&kind("fetchFromBitbucket"), &p, "v2.0.0");
        assert_eq!(url, "https://bitbucket.org/o/r/get/refs/tags/v2.0.0.tar.gz");
    }

    #[test]
    fn test_build_tarball_url_gitiles_tag_is_refs_tags() {
        let mut p = ParsedAttrs::default();
        p.strings
            .insert("url".to_string(), "https://g.example/repo".to_string());
        p.strings.insert("tag".to_string(), "v1.0.0".to_string());
        let url = archive_url(&kind("fetchFromGitiles"), &p, "v2.0.0");
        assert_eq!(
            url,
            "https://g.example/repo/+archive/refs/tags/v2.0.0.tar.gz"
        );
    }
}
