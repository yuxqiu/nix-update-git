//! Registry of git hosting forges (GitHub, GitLab, Gitea-likes, SourceHut,
//! Bitbucket, RepoOrCz, Gitiles).
//!
//! Each forge is one small `Forge` impl registered in `FORGES`. Adding a new
//! forge means adding one file and one line here — nothing in
//! `rules/fetcher/{kind,mod,tarball}.rs` or `rules/flake_input.rs` needs to
//! change, since those all dispatch through this trait generically.
//!
//! `fetchgit`, `builtins.fetchGit`, `fetchpatch`, and `fetchTarball` are
//! deliberately not forges: they identify their target purely by a raw
//! `url` attribute (any host already works, by construction) rather than by
//! `owner`/`repo`, so there is no "add a new one" need for them the way
//! there is for forges.

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
/// *which* repo (owner/repo/domain-ish), which each forge lists in
/// `extra_attrs()`.
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

    /// The flake input URL scheme this forge supports (`github:owner/repo`
    /// uses `"github"`), if any.
    fn flake_scheme(&self) -> Option<&'static str> {
        None
    }

    /// Attribute keys beyond `COMMON_GIT_ATTRS` — whatever identifies the
    /// repo for this forge (owner/repo/domain-ish keys).
    fn extra_attrs(&self) -> &'static [AttrSpec];

    /// HTTPS remote URL for `git ls-remote`, resolved from parsed
    /// fetcher/derivation attrs.
    fn git_url(&self, parsed: &ParsedAttrs) -> Option<String>;

    /// Same information without the scheme — what's shown to the user.
    fn display_target(&self, parsed: &ParsedAttrs) -> Option<String>;

    /// Tarball/archive download URL for a given rev (used by the `Tarball`
    /// hash strategy).
    fn archive_url(&self, parsed: &ParsedAttrs, rev: &str) -> Result<String>;

    /// HTTPS remote URL for the `scheme:owner/repo` flake shorthand, which
    /// carries no domain-override attrs. Only called when
    /// `flake_scheme()` is `Some`.
    fn remote_url_for_flake(&self, owner: &str, repo: &str) -> String {
        let _ = (owner, repo);
        unreachable!("{} has no flake scheme", self.id())
    }

    /// Same, for display. Only called when `flake_scheme()` is `Some`.
    fn display_for_flake(&self, owner: &str, repo: &str) -> String {
        let _ = (owner, repo);
        unreachable!("{} has no flake scheme", self.id())
    }

    /// Full attribute schema: `COMMON_GIT_ATTRS` plus this forge's own.
    fn attr_spec(&self) -> Vec<AttrSpec> {
        COMMON_GIT_ATTRS
            .iter()
            .chain(self.extra_attrs())
            .cloned()
            .collect()
    }
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

pub fn find_by_fn_name(name: &str) -> Option<&'static dyn Forge> {
    FORGES.iter().copied().find(|f| f.nixpkgs_fn_name() == name)
}

pub fn find_by_flake_scheme(scheme: &str) -> Option<&'static dyn Forge> {
    FORGES
        .iter()
        .copied()
        .find(|f| f.flake_scheme() == Some(scheme))
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
