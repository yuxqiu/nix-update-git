use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

use super::{Forge, ensure_tilde};

pub struct SourceHut;

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
    AttrSpec {
        key: "vc",
        attr_type: AttrType::String,
    },
];

impl Forge for SourceHut {
    fn id(&self) -> &'static str {
        "sourcehut"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromSourcehut"
    }

    fn flake_scheme(&self) -> Option<&'static str> {
        Some("sourcehut")
    }

    fn extra_attrs(&self) -> &'static [AttrSpec] {
        EXTRA_ATTRS
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let owner = ensure_tilde(owner);
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("sr.ht");
        let vc = parsed
            .strings
            .get("vc")
            .map(|s| s.as_str())
            .unwrap_or("git");
        Some(format!("https://{}.{}/{}/{}", vc, domain, owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let owner = ensure_tilde(owner);
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("sr.ht");
        let vc = parsed
            .strings
            .get("vc")
            .map(|s| s.as_str())
            .unwrap_or("git");
        Some(format!("{}.{}/{}/{}", vc, domain, owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let owner = parsed
            .strings
            .get("owner")
            .context("missing 'owner' parameter for fetchFromSourcehut")?;
        let owner = ensure_tilde(owner);
        let repo = parsed
            .strings
            .get("repo")
            .context("missing 'repo' parameter for fetchFromSourcehut")?;
        let domain = parsed
            .strings
            .get("domain")
            .map(|s| s.as_str())
            .unwrap_or("sr.ht");
        let vc = parsed
            .strings
            .get("vc")
            .map(|s| s.as_str())
            .unwrap_or("git");
        Ok(format!(
            "https://{}.{}/{}/{}/archive/{}.tar.gz",
            vc, domain, owner, repo, rev
        ))
    }

    // NOTE: preserved verbatim from the pre-refactor `FlakeUrl::SourceHut`
    // impl, including two pre-existing quirks that are out of scope for
    // this refactor: `remote_url_for_flake` always prepends `~` (so an
    // `owner` that already includes it, e.g. parsed from
    // `sourcehut:~user/repo`, doubles up), and it resolves against `sr.ht`
    // rather than `git.sr.ht` like everywhere else in this file.
    // `display_for_flake` does not have either issue.
    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("https://sr.ht/~{}/{}", owner, repo)
    }

    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        let owner = ensure_tilde(owner);
        format!("git.sr.ht/{}/{}", owner, repo)
    }
}
