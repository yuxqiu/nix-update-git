use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

use super::Forge;

pub struct RepoOrCz;

const EXTRA_ATTRS: &[AttrSpec] = &[AttrSpec {
    key: "repo",
    attr_type: AttrType::String,
}];

impl Forge for RepoOrCz {
    fn id(&self) -> &'static str {
        "repoorcz"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromRepoOrCz"
    }

    fn extra_attrs(&self) -> &'static [AttrSpec] {
        EXTRA_ATTRS
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let repo = parsed.strings.get("repo")?;
        Some(format!("https://repo.or.cz/{}.git", repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let repo = parsed.strings.get("repo")?;
        Some(format!("repo.or.cz/{}.git", repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromRepoOrCz")?;
        Ok(format!(
            "https://repo.or.cz/{}.git/snapshot/{}.tar.gz",
            repo, rev
        ))
    }
}
