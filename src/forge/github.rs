use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::{FlakeForge, Forge};

pub struct GitHub;

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
        key: "githubBase",
        attr_type: AttrType::String,
    },
];

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for GitHub {
    fn id(&self) -> &'static str {
        "github"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitHub"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        let base = parsed
            .strings
            .get("githubBase")
            .map(|s| s.as_str())
            .unwrap_or("github.com");
        Some(format!("https://{}/{}/{}", base, owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        // Always shows "github.com", even with a custom `githubBase` —
        // preserved from the pre-refactor behavior.
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        Some(format!("github.com/{}/{}", owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromGitHub")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromGitHub")?;
        let base = parsed
            .strings
            .get("githubBase")
            .map(|s| s.as_str())
            .unwrap_or("github.com");
        Ok(format!(
            "https://{}/{}/{}/archive/{}.tar.gz",
            base, owner, repo, rev
        ))
    }
}

impl FlakeForge for GitHub {
    fn flake_scheme(&self) -> &'static str {
        "github"
    }

    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("https://github.com/{}/{}", owner, repo)
    }

    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("github.com/{}/{}", owner, repo)
    }
}
