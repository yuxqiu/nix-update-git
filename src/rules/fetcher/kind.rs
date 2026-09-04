use std::fmt;
use std::sync::LazyLock;

use crate::forge::{self, Forge, strip_url_scheme};
use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use crate::rules::fetcher::source_url::parse_source_url;

/// A fetcher kind. Most variants wrap a registered `Forge`.
///
/// See `crate::forge` for the forge-vs-plain-variant rule; `FetchGit`,
/// `BuiltinsFetchGit`, `FetchPatch`, and `FetchTarball` aren't
/// `fetchFrom<Name>`-named, so they stay as their own variants here instead.
#[derive(Clone, Copy)]
pub enum FetcherKind {
    Forge(&'static dyn Forge),
    FetchGit,
    BuiltinsFetchGit,
    FetchPatch,
    FetchTarball,
}

impl fmt::Debug for FetcherKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl PartialEq for FetcherKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Relies on every registered forge's `id()` being unique —
            // enforced by a test in `forge::tests`, not by the type system.
            (Self::Forge(a), Self::Forge(b)) => a.id() == b.id(),
            (Self::FetchGit, Self::FetchGit)
            | (Self::BuiltinsFetchGit, Self::BuiltinsFetchGit)
            | (Self::FetchPatch, Self::FetchPatch)
            | (Self::FetchTarball, Self::FetchTarball) => true,
            _ => false,
        }
    }
}

impl Eq for FetcherKind {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashStrategy {
    Tarball,
    Git,
    None,
    Patch,
}

/// `fetchgit`'s attributes beyond the common git-fetcher base: a raw `url`
/// (rather than owner/repo), plus a legacy `submodules` alias for
/// `fetchSubmodules` that only this fetcher supports.
const FETCH_GIT_EXTRA_ATTRS: &[AttrSpec] = &[
    AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "submodules",
        attr_type: AttrType::Bool,
    },
];

static FETCH_GIT_ATTR_SPEC: LazyLock<Vec<AttrSpec>> =
    LazyLock::new(|| forge::compose_attr_spec(FETCH_GIT_EXTRA_ATTRS));

const SPEC_BUILTINS_FETCH_GIT: &[AttrSpec] = &[
    AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    },
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
        key: "submodules",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "sparseCheckout",
        attr_type: AttrType::ListString,
    },
];

const SPEC_FETCH_PATCH: &[AttrSpec] = &[
    AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "urls",
        attr_type: AttrType::ListString,
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
        key: "sha1",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "sha512",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "outputHash",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "outputHashAlgo",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "name",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "pname",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "version",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "stripLen",
        attr_type: AttrType::Int,
    },
    AttrSpec {
        key: "relative",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "extraPrefix",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "revert",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "excludes",
        attr_type: AttrType::ListString,
    },
    AttrSpec {
        key: "includes",
        attr_type: AttrType::ListString,
    },
    AttrSpec {
        key: "hunks",
        attr_type: AttrType::ListInt,
    },
    AttrSpec {
        key: "decode",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "postFetch",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "curlOpts",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "curlOptsList",
        attr_type: AttrType::ListString,
    },
    AttrSpec {
        key: "downloadToTemp",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "executable",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "showURLs",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "recursiveHash",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "netrcPhase",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "netrcImpureEnvVars",
        attr_type: AttrType::ListString,
    },
];

const SPEC_FETCH_TARBALL: &[AttrSpec] = &[
    AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "urls",
        attr_type: AttrType::ListString,
    },
    AttrSpec {
        key: "sha256",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "hash",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "name",
        attr_type: AttrType::String,
    },
];

impl FetcherKind {
    pub fn from_name(name: &str) -> Option<Self> {
        let short_name = name.rsplit('.').next().unwrap_or(name);

        match short_name {
            "fetchgit" | "fetchgitPrivate" => Some(Self::FetchGit),
            "fetchGit" => Some(Self::BuiltinsFetchGit),
            "fetchpatch" => Some(Self::FetchPatch),
            "fetchTarball" => Some(Self::FetchTarball),
            _ => forge::find_by_fn_name(short_name).map(Self::Forge),
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Forge(forge) => forge.nixpkgs_fn_name(),
            Self::FetchGit => "fetchgit",
            Self::BuiltinsFetchGit => "builtins.fetchGit",
            Self::FetchPatch => "fetchpatch",
            Self::FetchTarball => "fetchTarball",
        }
    }

    #[must_use]
    pub const fn needs_hash(&self) -> bool {
        !matches!(self, Self::BuiltinsFetchGit)
    }

    #[must_use]
    pub fn hash_strategy(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> HashStrategy {
        match self {
            Self::BuiltinsFetchGit => HashStrategy::None,
            Self::FetchPatch => HashStrategy::Patch,
            Self::FetchTarball => HashStrategy::Tarball,
            Self::FetchGit => HashStrategy::Git,
            Self::Forge(_) => {
                if self.uses_tarball(parsed, has_sparse_checkout) {
                    HashStrategy::Tarball
                } else {
                    HashStrategy::Git
                }
            }
        }
    }

    #[must_use]
    pub fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        match self {
            Self::FetchGit | Self::FetchPatch | Self::FetchTarball | Self::BuiltinsFetchGit => {
                parsed.strings.get("url").cloned()
            }
            Self::Forge(forge) => forge.git_url(parsed),
        }
    }

    #[must_use]
    pub fn uses_tarball(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> bool {
        !self.uses_fetchgit(parsed, has_sparse_checkout) && !self.uses_fetch_submodules(parsed)
    }

    #[must_use]
    pub fn uses_fetch_submodules(&self, parsed: &ParsedAttrs) -> bool {
        match self {
            Self::FetchGit | Self::Forge(_) => {
                parsed.bools.get("fetchSubmodules").is_some_and(|&v| v)
            }
            Self::BuiltinsFetchGit => parsed.bools.get("submodules").is_some_and(|&v| v),
            Self::FetchPatch | Self::FetchTarball => false,
        }
    }

    fn uses_fetchgit(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> bool {
        match self {
            Self::FetchGit | Self::BuiltinsFetchGit => true,
            Self::Forge(_) => {
                parsed.bools.get("forceFetchGit").is_some_and(|&v| v)
                    || parsed.bools.get("leaveDotGit").is_some_and(|&v| v)
                    || parsed.bools.get("deepClone").is_some_and(|&v| v)
                    || parsed.bools.get("fetchLFS").is_some_and(|&v| v)
                    || parsed.bools.get("fetchSubmodules").is_some_and(|&v| v)
                    || parsed.strings.get("rootDir").is_some_and(|v| !v.is_empty())
                    || has_sparse_checkout
            }
            Self::FetchPatch | Self::FetchTarball => false,
        }
    }

    #[must_use]
    pub fn attr_spec(&self) -> &'static [AttrSpec] {
        match self {
            Self::Forge(forge) => forge.attr_spec(),
            Self::FetchGit => &FETCH_GIT_ATTR_SPEC,
            Self::BuiltinsFetchGit => SPEC_BUILTINS_FETCH_GIT,
            Self::FetchPatch => SPEC_FETCH_PATCH,
            Self::FetchTarball => SPEC_FETCH_TARBALL,
        }
    }

    #[must_use]
    pub fn operational_keys(&self) -> Vec<&'static str> {
        self.attr_spec().iter().map(|s| s.key).collect()
    }

    #[must_use]
    pub fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        match self {
            Self::Forge(forge) => forge.display_target(parsed),
            Self::FetchPatch | Self::FetchTarball => {
                let url = parsed.strings.get("url").cloned().or_else(|| {
                    parsed
                        .pure_string_list("urls")
                        .and_then(|urls| urls.into_iter().next())
                })?;
                // Prefer the structured "domain/project" form for recognized
                // hosts; fall back to the raw URL (still useful context) for
                // self-hosted/unrecognized ones rather than showing nothing.
                Some(parse_source_url(&url).map_or_else(
                    || strip_url_scheme(&url).to_string(),
                    |p| format!("{}/{}", p.domain, p.project),
                ))
            }
            Self::FetchGit | Self::BuiltinsFetchGit => parsed
                .strings
                .get("url")
                .map(|url| strip_url_scheme(url).to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(fn_name: &str) -> FetcherKind {
        FetcherKind::from_name(fn_name).unwrap()
    }

    #[test]
    fn test_fetcher_kind_from_name() {
        assert_eq!(
            FetcherKind::from_name("fetchFromGitHub"),
            Some(kind("fetchFromGitHub"))
        );
        assert_eq!(
            FetcherKind::from_name("fetchgit"),
            Some(FetcherKind::FetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("fetchGit"),
            Some(FetcherKind::BuiltinsFetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("builtins.fetchGit"),
            Some(FetcherKind::BuiltinsFetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("pkgs.fetchFromGitHub"),
            Some(kind("fetchFromGitHub"))
        );
        assert_eq!(
            FetcherKind::from_name("pkgs.fetchgit"),
            Some(FetcherKind::FetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("lib.fetchFromGitLab"),
            Some(kind("fetchFromGitLab"))
        );

        assert_eq!(
            FetcherKind::from_name("fetchgitPrivate"),
            Some(FetcherKind::FetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("fetchpatch"),
            Some(FetcherKind::FetchPatch)
        );
        assert_eq!(
            FetcherKind::from_name("pkgs.fetchpatch"),
            Some(FetcherKind::FetchPatch)
        );
        assert_eq!(
            FetcherKind::from_name("fetchTarball"),
            Some(FetcherKind::FetchTarball)
        );
        assert_eq!(FetcherKind::from_name("unknown"), None);
        assert_eq!(FetcherKind::from_name("pkgs.unknown"), None);
    }

    #[test]
    fn test_fetcher_git_url_github() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "NixOS".to_string());
        params
            .strings
            .insert("repo".to_string(), "nixpkgs".to_string());
        let url = kind("fetchFromGitHub").git_url(&params).unwrap();
        assert_eq!(url, "https://github.com/NixOS/nixpkgs");
    }

    #[test]
    fn test_fetcher_git_url_gitlab() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "foo".to_string());
        params.strings.insert("repo".to_string(), "bar".to_string());
        let url = kind("fetchFromGitLab").git_url(&params).unwrap();
        assert_eq!(url, "https://gitlab.com/foo/bar");
    }

    #[test]
    fn test_fetcher_git_url_gitea() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("domain".to_string(), "gitea.example.com".to_string());
        params
            .strings
            .insert("owner".to_string(), "foo".to_string());
        params.strings.insert("repo".to_string(), "bar".to_string());
        let url = kind("fetchFromGitea").git_url(&params).unwrap();
        assert_eq!(url, "https://gitea.example.com/foo/bar");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "~sirhc".to_string());
        params
            .strings
            .insert("repo".to_string(), "repo".to_string());
        let url = kind("fetchFromSourcehut").git_url(&params).unwrap();
        assert_eq!(url, "https://git.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_no_tilde() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "sirhc".to_string());
        params
            .strings
            .insert("repo".to_string(), "repo".to_string());
        let url = kind("fetchFromSourcehut").git_url(&params).unwrap();
        assert_eq!(url, "https://git.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_custom_domain() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "~sirhc".to_string());
        params
            .strings
            .insert("repo".to_string(), "repo".to_string());
        params
            .strings
            .insert("domain".to_string(), "custom.sr.ht".to_string());
        let url = kind("fetchFromSourcehut").git_url(&params).unwrap();
        assert_eq!(url, "https://git.custom.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_custom_vc() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "~sirhc".to_string());
        params
            .strings
            .insert("repo".to_string(), "repo".to_string());
        params.strings.insert("vc".to_string(), "hg".to_string());
        let url = kind("fetchFromSourcehut").git_url(&params).unwrap();
        assert_eq!(url, "https://hg.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_repo_or_cz() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("repo".to_string(), "testrepo".to_string());
        let url = kind("fetchFromRepoOrCz").git_url(&params).unwrap();
        assert_eq!(url, "https://repo.or.cz/testrepo.git");
    }

    #[test]
    fn test_fetcher_git_url_builtins_fetch_git() {
        let mut params = ParsedAttrs::default();
        params.strings.insert(
            "url".to_string(),
            "https://example.com/repo.git".to_string(),
        );
        let url = FetcherKind::BuiltinsFetchGit.git_url(&params);
        assert_eq!(url, Some("https://example.com/repo.git".to_string()));
    }

    #[test]
    fn test_fetcher_git_url_fetchgit_with_url() {
        let mut params = ParsedAttrs::default();
        params.strings.insert(
            "url".to_string(),
            "https://example.com/repo.git".to_string(),
        );
        let url = FetcherKind::FetchGit.git_url(&params);
        assert_eq!(url, Some("https://example.com/repo.git".to_string()));
    }

    #[test]
    fn test_fetcher_git_url_gitiles() {
        let mut params = ParsedAttrs::default();
        params.strings.insert(
            "url".to_string(),
            "https://android.googlesource.com/platform/manifest".to_string(),
        );
        let url = kind("fetchFromGitiles").git_url(&params);
        assert_eq!(
            url,
            Some("https://android.googlesource.com/platform/manifest".to_string())
        );
    }

    #[test]
    fn test_fetcher_git_url_bitbucket() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "testowner".to_string());
        params
            .strings
            .insert("repo".to_string(), "testrepo".to_string());
        let url = kind("fetchFromBitbucket").git_url(&params).unwrap();
        assert_eq!(url, "https://bitbucket.org/testowner/testrepo");
    }

    #[test]
    fn test_uses_fetch_submodules_true_fetchsubmodules() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(FetcherKind::FetchGit.uses_fetch_submodules(&params));
        for name in [
            "fetchFromGitHub",
            "fetchFromGitLab",
            "fetchFromGitea",
            "fetchFromForgejo",
            "fetchFromCodeberg",
            "fetchFromBitbucket",
            "fetchFromSourcehut",
            "fetchFromGitiles",
            "fetchFromRepoOrCz",
        ] {
            assert!(kind(name).uses_fetch_submodules(&params));
        }
    }

    #[test]
    fn test_uses_fetch_submodules_false() {
        let params = ParsedAttrs::default();
        assert!(!kind("fetchFromGitHub").uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_builtins_fetch_git() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("submodules".to_string(), true);
        assert!(FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
        assert!(!kind("fetchFromGitHub").uses_fetch_submodules(&params));

        let mut params = ParsedAttrs::default();
        params.bools.insert("submodules".to_string(), false);
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetchgit_always_true() {
        let params = ParsedAttrs::default();
        assert!(FetcherKind::FetchGit.uses_fetchgit(&params, false));
        assert!(FetcherKind::BuiltinsFetchGit.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_force_fetch_git() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), true);
        for name in [
            "fetchFromGitHub",
            "fetchFromGitLab",
            "fetchFromGitea",
            "fetchFromForgejo",
            "fetchFromCodeberg",
            "fetchFromBitbucket",
            "fetchFromSourcehut",
            "fetchFromGitiles",
            "fetchFromRepoOrCz",
        ] {
            assert!(kind(name).uses_fetchgit(&params, false));
        }
    }

    #[test]
    fn test_uses_fetchgit_leave_dot_git() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("leaveDotGit".to_string(), true);
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_deep_clone() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), true);
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_lfs() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchLFS".to_string(), true);
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_submodules() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_root_dir() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("rootDir".to_string(), "/some/path".to_string());
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.strings.insert("rootDir".to_string(), String::new());
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_sparse_checkout() {
        let params = ParsedAttrs::default();
        assert!(kind("fetchFromGitHub").uses_fetchgit(&params, true));
        assert!(kind("fetchFromGitLab").uses_fetchgit(&params, true));
        assert!(kind("fetchFromCodeberg").uses_fetchgit(&params, true));

        let params = ParsedAttrs::default();
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));
        assert!(!kind("fetchFromGitLab").uses_fetchgit(&params, false));
        assert!(!kind("fetchFromCodeberg").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_false_when_no_trigger_params() {
        let params = ParsedAttrs::default();
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));
        assert!(!kind("fetchFromGitLab").uses_fetchgit(&params, false));
        assert!(!kind("fetchFromCodeberg").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("leaveDotGit".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchLFS".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), false);
        assert!(!kind("fetchFromGitHub").uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_tarball_supported_fetchers() {
        let params = ParsedAttrs::default();
        for name in [
            "fetchFromGitHub",
            "fetchFromGitLab",
            "fetchFromGitea",
            "fetchFromForgejo",
            "fetchFromCodeberg",
            "fetchFromSourcehut",
            "fetchFromBitbucket",
            "fetchFromGitiles",
            "fetchFromRepoOrCz",
        ] {
            assert!(kind(name).uses_tarball(&params, false));
        }
        assert!(!FetcherKind::FetchGit.uses_tarball(&params, false));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_fetchgit_flags() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), true);
        assert!(!kind("fetchFromGitHub").uses_tarball(&params, false));
        assert!(!kind("fetchFromGitLab").uses_tarball(&params, false));
        assert!(!kind("fetchFromCodeberg").uses_tarball(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(!kind("fetchFromGitHub").uses_tarball(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), true);
        assert!(!kind("fetchFromGitHub").uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_sparse_checkout() {
        let params = ParsedAttrs::default();
        assert!(!kind("fetchFromGitHub").uses_tarball(&params, true));
        assert!(!kind("fetchFromGitLab").uses_tarball(&params, true));
        assert!(!kind("fetchFromCodeberg").uses_tarball(&params, true));
    }

    #[test]
    fn test_hash_strategy() {
        let params = ParsedAttrs::default();
        assert_eq!(
            kind("fetchFromGitHub").hash_strategy(&params, false),
            HashStrategy::Tarball
        );
        assert_eq!(
            FetcherKind::FetchGit.hash_strategy(&params, false),
            HashStrategy::Git
        );
        assert_eq!(
            FetcherKind::BuiltinsFetchGit.hash_strategy(&params, false),
            HashStrategy::None
        );

        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), true);
        assert_eq!(
            kind("fetchFromGitHub").hash_strategy(&params, false),
            HashStrategy::Git
        );
    }

    #[test]
    fn test_fetchpatch_display_target_uses_domain_project_for_recognized_host() {
        let mut params = ParsedAttrs::default();
        params.strings.insert(
            "url".to_string(),
            "https://github.com/owner/repo/commit/abc123.patch".to_string(),
        );
        assert_eq!(
            FetcherKind::FetchPatch.display_target(&params),
            Some("github.com/owner/repo".to_string())
        );
    }

    #[test]
    fn test_fetchpatch_display_target_falls_back_to_raw_url_for_unrecognized_host() {
        // Regression test: a self-hosted/unrecognized URL used to make
        // display_target return None, silently dropping the hunk header
        // instead of showing the URL as context.
        let mut params = ParsedAttrs::default();
        params.strings.insert(
            "url".to_string(),
            "https://patches.example.internal/some/weird/path.patch".to_string(),
        );
        assert_eq!(
            FetcherKind::FetchPatch.display_target(&params),
            Some("patches.example.internal/some/weird/path.patch".to_string())
        );
    }

    #[test]
    fn test_fetchtarball_display_target_none_without_any_url() {
        let params = ParsedAttrs::default();
        assert_eq!(FetcherKind::FetchTarball.display_target(&params), None);
    }
}
