use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::Forge;

pub struct RepoOrCz;

const EXTRA_ATTRS: &[AttrSpec] = &[AttrSpec {
    key: "repo",
    attr_type: AttrType::String,
}];

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for RepoOrCz {
    fn id(&self) -> &'static str {
        "repoorcz"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromRepoOrCz"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
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
