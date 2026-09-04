use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::Forge;

pub struct Bitbucket;

const EXTRA_ATTRS: &[AttrSpec] = &[
    AttrSpec {
        key: "owner",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "repo",
        attr_type: AttrType::String,
    },
];

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for Bitbucket {
    fn id(&self) -> &'static str {
        "bitbucket"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromBitbucket"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("https://bitbucket.org/{}/{}", owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("bitbucket.org/{}/{}", owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromBitbucket")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromBitbucket")?;
        Ok(format!(
            "https://bitbucket.org/{}/{}/get/{}.tar.gz",
            owner,
            repo,
            super::tag_or_rev(parsed, rev)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> ParsedAttrs {
        let mut parsed = ParsedAttrs::default();
        for (k, v) in pairs {
            parsed.strings.insert(k.to_string(), v.to_string());
        }
        parsed
    }

    #[test]
    fn test_archive_url_uses_bare_rev_without_tag() {
        let parsed = attrs(&[("owner", "foo"), ("repo", "bar")]);
        assert_eq!(
            Bitbucket.archive_url(&parsed, "abc123").unwrap(),
            "https://bitbucket.org/foo/bar/get/abc123.tar.gz"
        );
    }

    #[test]
    fn test_archive_url_uses_refs_tags_path_with_tag() {
        let parsed = attrs(&[("owner", "foo"), ("repo", "bar"), ("tag", "v1.0.0")]);
        assert_eq!(
            Bitbucket.archive_url(&parsed, "v1.0.0").unwrap(),
            "https://bitbucket.org/foo/bar/get/refs/tags/v1.0.0.tar.gz"
        );
    }
}
