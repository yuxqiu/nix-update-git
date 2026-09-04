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

    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        let owner = ensure_tilde(owner);
        format!("https://git.sr.ht/{}/{}", owner, repo)
    }

    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        let owner = ensure_tilde(owner);
        format!("git.sr.ht/{}/{}", owner, repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_url_for_flake_adds_tilde_once() {
        assert_eq!(
            SourceHut.remote_url_for_flake("sirhc", "repo"),
            "https://git.sr.ht/~sirhc/repo"
        );
    }

    #[test]
    fn test_remote_url_for_flake_does_not_double_tilde() {
        assert_eq!(
            SourceHut.remote_url_for_flake("~sirhc", "repo"),
            "https://git.sr.ht/~sirhc/repo"
        );
    }

    #[test]
    fn test_display_for_flake_matches_remote_url_shape() {
        assert_eq!(
            SourceHut.display_for_flake("~sirhc", "repo"),
            "git.sr.ht/~sirhc/repo"
        );
    }
}
