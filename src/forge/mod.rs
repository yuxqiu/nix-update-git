//! Registry of git hosting forges (GitHub, GitLab, Gitea-likes, SourceHut,
//! Bitbucket, RepoOrCz, Gitiles).
//!
//! Each forge is one small `Forge` impl registered in `FORGES`. Adding a new
//! forge means adding one file and one line here — nothing in
//! `rules/fetcher/{kind,mod,tarball}.rs` or `rules/flake_input.rs` needs to
//! change, since those all dispatch through this trait generically.
//!
//! The rule for what's a forge: does the nixpkgs fetcher function name
//! follow the `fetchFrom<Name>` convention (`fetchFromGitHub`,
//! `fetchFromGitiles`, ...)? If so, it's a `Forge`. `fetchgit`,
//! `builtins.fetchGit`, `fetchpatch`, and `fetchTarball` don't, and stay as
//! plain `FetcherKind` variants instead — this is nixpkgs' own naming
//! convention for host-specific vs. generic fetchers, not something
//! invented here.

mod bitbucket;
mod codeberg;
mod forgejo;
mod gitea;
mod github;
mod gitiles;
mod gitlab;
mod repoorcz;
mod sourcehut;

use anyhow::Result;

use crate::parser::{AttrSpec, AttrType, ParsedAttrs};

/// Attribute keys shared by every forge's fetcher call (`fetchFromGitHub`,
/// `fetchFromGitLab`, ...): everything except the keys that identify
/// *which* repo (owner/repo/domain-ish), which each forge composes in via
/// `compose_attr_spec()`.
pub const COMMON_GIT_ATTRS: &[AttrSpec] = &[
    AttrSpec {
        key: "tag",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "rev",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "ref",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "hash",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "sha256",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "fetchSubmodules",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "deepClone",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "leaveDotGit",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "fetchLFS",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "branchName",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "rootDir",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "sparseCheckout",
        attr_type: AttrType::ListString,
    },
    AttrSpec {
        key: "forceFetchGit",
        attr_type: AttrType::Bool,
    },
];

/// A git hosting forge reachable through a `fetchFromX`-style nixpkgs
/// fetcher (or, for `Gitiles`, a `url`-keyed equivalent — see the module
/// doc for why it's still a `Forge`).
pub trait Forge: Sync {
    /// Stable identifier, used for equality/debug and dispatch.
    fn id(&self) -> &'static str;

    /// The nixpkgs fetcher function name this forge is invoked through,
    /// e.g. `"fetchFromGitHub"`.
    fn nixpkgs_fn_name(&self) -> &'static str;

    /// HTTPS remote URL for `git ls-remote`, resolved from parsed
    /// fetcher/derivation attrs.
    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String>;

    /// Same information without the scheme — what's shown to the user.
    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String>;

    /// Tarball/archive download URL for a given rev (used by the `Tarball`
    /// hash strategy).
    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String>;

    /// Full attribute schema: `COMMON_GIT_ATTRS` plus this forge's own
    /// (owner/repo/domain-ish keys). Not a default method: each impl caches
    /// its own composed spec in a `LazyLock` (via `compose_attr_spec`) so
    /// this stays a zero-allocation `&'static` lookup rather than
    /// reassembling the `Vec` on every call.
    fn attr_spec(&self) -> &'static [AttrSpec];
}

/// Composes `COMMON_GIT_ATTRS` with a forge's own attrs. Each `Forge` impl
/// calls this once, inside a `static ATTR_SPEC: LazyLock<Vec<AttrSpec>>`, to
/// implement `attr_spec()` — see any forge module for the pattern.
pub fn compose_attr_spec(extra: &'static [AttrSpec]) -> Vec<AttrSpec> {
    COMMON_GIT_ATTRS.iter().chain(extra).cloned().collect()
}

/// A `Forge` that also supports the `scheme:owner/repo` flake input
/// shorthand (`github:owner/repo`, `gitlab:owner/repo`, ...). A separate
/// trait rather than optional `Forge` methods: only some forges support
/// this, and callers that need it (`flake_input.rs`) should hold a
/// `&dyn FlakeForge` so the methods are provably callable — no runtime
/// check, no panic path for forges that don't support it.
pub trait FlakeForge: Forge {
    /// The scheme itself, e.g. `"github"` for `github:owner/repo`.
    fn flake_scheme(&self) -> &'static str;

    /// HTTPS remote URL for the flake shorthand, which carries no
    /// domain-override attrs (unlike `Forge::git_url`).
    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String;

    /// Same, for display.
    fn display_for_flake(&self, owner: &str, repo: &str) -> String;
}

/// All registered forges. Adding one means implementing `Forge` in its own
/// file and adding it here — no other file needs to change.
pub const FORGES: &[&dyn Forge] = &[
    &github::GitHub,
    &gitlab::GitLab,
    &gitea::Gitea,
    &forgejo::Forgejo,
    &codeberg::Codeberg,
    &sourcehut::SourceHut,
    &bitbucket::Bitbucket,
    &repoorcz::RepoOrCz,
    &gitiles::Gitiles,
];

/// The subset of `FORGES` that also implement `FlakeForge`. `flake_input.rs`
/// iterates this (not `FORGES`) to recognize `scheme:owner/repo` shorthand.
pub const FLAKE_FORGES: &[&dyn FlakeForge] =
    &[&github::GitHub, &gitlab::GitLab, &sourcehut::SourceHut];

pub fn find_by_fn_name(name: &str) -> Option<&'static dyn Forge> {
    FORGES.iter().copied().find(|f| f.nixpkgs_fn_name() == name)
}

/// Normalizes an owner for forges (currently only SourceHut) that require a
/// `~` prefix, adding it if missing.
pub(crate) fn ensure_tilde(owner: &str) -> String {
    if owner.starts_with('~') {
        owner.to_string()
    } else {
        format!("~{}", owner)
    }
}

/// Strips a known URL scheme prefix, for forges/kinds that display a raw
/// `url` attribute as-is rather than reformatting it (Gitiles, and the
/// non-forge `fetchgit`/`builtins.fetchGit` kinds).
pub(crate) fn strip_url_scheme(url: &str) -> &str {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git://"))
        .unwrap_or(url)
}

/// The path segment identifying `rev` in an archive URL: `refs/tags/<rev>`
/// when the call pins by `tag` (some hosts require the full ref path for
/// tag archives), otherwise the bare `rev`.
pub(crate) fn tag_or_rev(parsed: &ParsedAttrs, rev: &str) -> String {
    if parsed.strings.contains_key("tag") {
        format!("refs/tags/{}", rev)
    } else {
        rev.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// `FetcherKind`'s `PartialEq` compares forges by `id()` string, and
    /// `find_by_fn_name`/`find_by_scheme`-style lookups return the first
    /// match — both silently misbehave if two registered forges share an
    /// id or fetcher/scheme name (a copy-pasted new forge forgetting to
    /// change one of these would compile fine and fail silently). Assert
    /// uniqueness explicitly so that shows up as a test failure instead.
    #[test]
    fn test_forge_ids_are_unique() {
        let ids: HashSet<&str> = FORGES.iter().map(|f| f.id()).collect();
        assert_eq!(ids.len(), FORGES.len(), "duplicate Forge::id() in FORGES");
    }

    #[test]
    fn test_forge_fn_names_are_unique() {
        let names: HashSet<&str> = FORGES.iter().map(|f| f.nixpkgs_fn_name()).collect();
        assert_eq!(
            names.len(),
            FORGES.len(),
            "duplicate Forge::nixpkgs_fn_name() in FORGES"
        );
    }

    #[test]
    fn test_flake_forge_schemes_are_unique() {
        let schemes: HashSet<&str> = FLAKE_FORGES.iter().map(|f| f.flake_scheme()).collect();
        assert_eq!(
            schemes.len(),
            FLAKE_FORGES.len(),
            "duplicate FlakeForge::flake_scheme() in FLAKE_FORGES"
        );
    }

    #[test]
    fn test_tag_or_rev_uses_bare_rev_without_tag_key() {
        let parsed = ParsedAttrs::default();
        assert_eq!(tag_or_rev(&parsed, "v1.0.0"), "v1.0.0");
    }

    #[test]
    fn test_tag_or_rev_uses_refs_tags_path_with_tag_key() {
        let mut parsed = ParsedAttrs::default();
        parsed
            .strings
            .insert("tag".to_string(), "v1.0.0".to_string());
        assert_eq!(tag_or_rev(&parsed, "v1.0.0"), "refs/tags/v1.0.0");
    }
}
