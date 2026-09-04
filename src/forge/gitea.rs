use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

use super::Forge;

/// Also covers self-hosted Gitea-compatible instances that aren't Codeberg
/// or Forgejo specifically (those get their own `Forge` impls only because
/// they have distinct nixpkgs fetcher function names).
pub struct Gitea;

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

impl Forge for Gitea {
    fn id(&self) -> &'static str {
        "gitea"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitea"
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
            .context("missing 'domain' parameter for fetchFromGitea")?;
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromGitea")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromGitea")?;
        Ok(format!(
            "https://{}/{}/{}/archive/{}.tar.gz",
            domain, owner, repo, rev
        ))
    }
}
