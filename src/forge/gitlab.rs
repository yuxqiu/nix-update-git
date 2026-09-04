use anyhow::{Context, Result};

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use std::sync::LazyLock;

use super::{FlakeForge, Forge};

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

static ATTR_SPEC: LazyLock<Vec<AttrSpec>> = LazyLock::new(|| super::compose_attr_spec(EXTRA_ATTRS));

impl Forge for GitLab {
    fn id(&self) -> &'static str {
        "gitlab"
    }

    fn nixpkgs_fn_name(&self) -> &'static str {
        "fetchFromGitLab"
    }

    fn attr_spec(&self) -> &'static [AttrSpec] {
        &ATTR_SPEC
    }

    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map_or("gitlab.com", std::string::String::as_str);
        Some(format!("https://{}/{}/{}", domain, owner, repo))
    }

    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        let owner = parsed.strings.get("owner")?;
        let repo = parsed.strings.get("repo")?;
        let domain = parsed
            .strings
            .get("domain")
            .map_or("gitlab.com", std::string::String::as_str);
        Some(format!("{}/{}/{}", domain, owner, repo))
    }

    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String> {
        let domain = parsed
            .strings
            .get("domain")
            .map_or("gitlab.com", std::string::String::as_str);
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
}

impl FlakeForge for GitLab {
    fn flake_scheme(&self) -> &'static str {
        "gitlab"
    }

    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("https://gitlab.com/{}/{}", owner, repo)
    }

    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        format!("gitlab.com/{}/{}", owner, repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> ParsedAttrs {
        let mut parsed = ParsedAttrs::default();
        for (k, v) in pairs {
            parsed.strings.insert(k.to_string(), v.to_string());
        }
        parsed
    }

    #[test]
    fn test_git_url_defaults_to_gitlab_com() {
        let parsed = attrs(&[("owner", "foo"), ("repo", "bar")]);
        assert_eq!(
            GitLab.git_url(&parsed),
            Some("https://gitlab.com/foo/bar".to_string())
        );
    }

    #[test]
    fn test_git_url_honors_self_hosted_domain() {
        let parsed = attrs(&[
            ("owner", "foo"),
            ("repo", "bar"),
            ("domain", "gitlab.example.com"),
        ]);
        assert_eq!(
            GitLab.git_url(&parsed),
            Some("https://gitlab.example.com/foo/bar".to_string())
        );
    }

    #[test]
    fn test_display_target_honors_self_hosted_domain() {
        let parsed = attrs(&[
            ("owner", "foo"),
            ("repo", "bar"),
            ("domain", "gitlab.example.com"),
        ]);
        assert_eq!(
            GitLab.display_target(&parsed),
            Some("gitlab.example.com/foo/bar".to_string())
        );
    }

    #[test]
    fn test_archive_url_shape() {
        let parsed = attrs(&[("owner", "foo"), ("repo", "bar")]);
        assert_eq!(
            GitLab.archive_url(&parsed, "v1.0.0").unwrap(),
            "https://gitlab.com/foo/bar/-/archive/v1.0.0/bar-v1.0.0.tar.gz"
        );
    }
}
