use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::{Forge, strip_url_scheme};

/// `fetchFromGitiles` is url-keyed rather than owner/repo-keyed (any
/// Gitiles-compatible host already works without a registry entry per
/// host), but it participates in the same git-vs-tarball dispatch and
/// tarball-hashing machinery as the owner/repo forges, so it's still a
/// `Forge`.
pub struct Gitiles;

const EXTRA_ATTRS: &[AttrSpec] = &[AttrSpec {
    key: "url",
    attr_type: AttrType::String,
}];

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for Gitiles {
    fn id(&self) -> &'static str {
        "gitiles"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitiles"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        parsed.strings.get("url").cloned()
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        parsed
            .strings
            .get("url")
            .map(|url| strip_url_scheme(url).to_string())
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let base_url = parsed
            .strings
            .get("url")
            .context("missing 'url' parameter for fetchFromGitiles")?;
        Ok(format!(
            "{}/+archive/{}.tar.gz",
            base_url,
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
    fn test_display_target_strips_scheme() {
        let parsed = attrs(&[("url", "https://gerrit.googlesource.com/git-repo")]);
        assert_eq!(
            Gitiles.display_target(&parsed),
            Some("gerrit.googlesource.com/git-repo".to_string())
        );
    }

    #[test]
    fn test_archive_url_uses_bare_rev_without_tag() {
        let parsed = attrs(&[("url", "https://gerrit.googlesource.com/git-repo")]);
        assert_eq!(
            Gitiles.archive_url(&parsed, "abc123").unwrap(),
            "https://gerrit.googlesource.com/git-repo/+archive/abc123.tar.gz"
        );
    }

    #[test]
    fn test_archive_url_uses_refs_tags_path_with_tag() {
        let parsed = attrs(&[
            ("url", "https://gerrit.googlesource.com/git-repo"),
            ("tag", "v1.0.0"),
        ]);
        assert_eq!(
            Gitiles.archive_url(&parsed, "v1.0.0").unwrap(),
            "https://gerrit.googlesource.com/git-repo/+archive/refs/tags/v1.0.0.tar.gz"
        );
    }
}
