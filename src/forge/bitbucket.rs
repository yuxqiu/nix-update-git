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
}
