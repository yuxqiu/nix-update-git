use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::Forge;

pub struct Codeberg;

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

impl Forge for Codeberg {
    fn id(&self) -> &'static str {
        "codeberg"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromCodeberg"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("https://codeberg.org/{}/{}", owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("codeberg.org/{}/{}", owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromCodeberg")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromCodeberg")?;
        Ok(format!(
            "https://codeberg.org/{}/{}/archive/{}.tar.gz",
            owner, repo, rev
        ))
    }
}
