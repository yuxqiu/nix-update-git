use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

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

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for Gitea {
    fn id(&self) -> &'static str {
        "gitea"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitea"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
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
