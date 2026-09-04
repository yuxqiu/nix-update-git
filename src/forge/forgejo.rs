use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

use super::Forge;

pub struct Forgejo;

const EXTRA_ATTRS: &[AttrSpec] = &[
    AttrSpec {
        key: "owner",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "repo",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "domain",
        attr_type: AttrType::String,
    },
];

impl Forge for Forgejo {
    fn id(&self) -> &'static str {
        "forgejo"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromForgejo"
    }

    fn extra_attrs(&self) -> &'static [AttrSpec] {
        EXTRA_ATTRS
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let domain = parsed.strings.get("domain")?;
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("https://{}/{}/{}", domain, owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let domain = parsed.strings.get("domain")?;
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("{}/{}/{}", domain, owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let domain = parsed
            .strings
            .get("domain")
            .context("missing 'domain' parameter for fetchFromForgejo")?;
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromForgejo")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromForgejo")?;
        Ok(format!(
            "https://{}/{}/{}/archive/{}.tar.gz",
            domain, owner, repo, rev
        ))
    }
}
