use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

use super::Forge;

pub struct GitLab;

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

impl Forge for GitLab {
    fn id(&self) -> &'static str {
        "gitlab"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitLab"
    }

    fn flake_scheme(&self) -> Option<&'static str> {
        Some("gitlab")
    }

    fn extra_attrs(&self) -> &'static [AttrSpec] {
        EXTRA_ATTRS
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("gitlab.com");
        Some(format!("https://{}/{}/{}", domain, owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("gitlab.com");
        Some(format!("{}/{}/{}", domain, owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("gitlab.com");
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromGitLab")?;
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromGitLab")?;
        Ok(format!(
            "https://{}/{}/{}/-/archive/{}/{}-{}.tar.gz",
            domain, owner, repo, rev, repo, rev
        ))
    }

    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("https://gitlab.com/{}/{}", owner, repo)
    }

    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("gitlab.com/{}/{}", owner, repo)
    }
}
