use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

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

impl Forge for Codeberg {
    fn id(&self) -> &'static str {
        "codeberg"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromCodeberg"
    }

    fn extra_attrs(&self) -> &'static [AttrSpec] {
        EXTRA_ATTRS
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
