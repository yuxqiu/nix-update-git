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
        let rev_or_tag = if parsed.strings.contains_key("tag") {
            format!("refs/tags/{}", rev)
        } else {
            rev.to_string()
        };
        Ok(format!("{}/+archive/{}.tar.gz", base_url, rev_or_tag))
    }
}
