use crate::parser::{AttrSpec, AttrType, ParsedAttrs};
use crate::rules::fetcher::source_url::parse_source_url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetcherKind {
    FetchGit,
    FetchFromGitHub,
    FetchFromGitLab,
    FetchFromGitea,
    FetchFromForgejo,
    FetchFromCodeberg,
    FetchFromSourcehut,
    FetchFromBitbucket,
    FetchFromGitiles,
    FetchFromRepoOrCz,
    BuiltinsFetchGit,
    FetchPatch,
    FetchTarball,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashStrategy {
    Tarball,
    Git,
    None,
    Patch,
}

impl FetcherKind {
    pub fn from_name(name: &str) -> Option<Self> {
        let short_name = name.rsplit('.').next().unwrap_or(name);

        match short_name {
            "fetchgit" | "fetchgitPrivate" => Some(Self::FetchGit),
            "fetchFromGitHub" => Some(Self::FetchFromGitHub),
            "fetchFromGitLab" => Some(Self::FetchFromGitLab),
            "fetchFromGitea" => Some(Self::FetchFromGitea),
            "fetchFromForgejo" => Some(Self::FetchFromForgejo),
            "fetchFromCodeberg" => Some(Self::FetchFromCodeberg),
            "fetchFromSourcehut" => Some(Self::FetchFromSourcehut),
            "fetchFromBitbucket" => Some(Self::FetchFromBitbucket),
            "fetchFromGitiles" => Some(Self::FetchFromGitiles),
            "fetchFromRepoOrCz" => Some(Self::FetchFromRepoOrCz),
            "fetchGit" => Some(Self::BuiltinsFetchGit),
            "fetchpatch" => Some(Self::FetchPatch),
            "fetchTarball" => Some(Self::FetchTarball),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::FetchGit => "fetchgit",
            Self::FetchFromGitHub => "fetchFromGitHub",
            Self::FetchFromGitLab => "fetchFromGitLab",
            Self::FetchFromGitea => "fetchFromGitea",
            Self::FetchFromForgejo => "fetchFromForgejo",
            Self::FetchFromCodeberg => "fetchFromCodeberg",
            Self::FetchFromSourcehut => "fetchFromSourcehut",
            Self::FetchFromBitbucket => "fetchFromBitbucket",
            Self::FetchFromGitiles => "fetchFromGitiles",
            Self::FetchFromRepoOrCz => "fetchFromRepoOrCz",
            Self::BuiltinsFetchGit => "builtins.fetchGit",
            Self::FetchPatch => "fetchpatch",
            Self::FetchTarball => "fetchTarball",
        }
    }

    pub fn needs_hash(&self) -> bool {
        !matches!(self, Self::BuiltinsFetchGit)
    }

    pub fn hash_strategy(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> HashStrategy {
        if !self.needs_hash() {
            return HashStrategy::None;
        }
        if matches!(self, Self::FetchPatch) {
            return HashStrategy::Patch;
        }
        if matches!(self, Self::FetchTarball) {
            return HashStrategy::Tarball;
        }
        if self.uses_tarball(parsed, has_sparse_checkout) {
            HashStrategy::Tarball
        } else {
            HashStrategy::Git
        }
    }

    pub fn git_url(&self, parsed: &ParsedAttrs) -> Option<String> {
        match self {
            Self::FetchGit | Self::FetchPatch | Self::FetchTarball => {
                parsed.strings.get("url").cloned()
            }
            Self::FetchFromGitHub => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                let base = parsed
                    .strings
                    .get("githubBase")
                    .map(|s| s.as_str())
                    .unwrap_or("github.com");
                Some(format!("https://{}/{}/{}", base, owner, repo))
            }
            Self::FetchFromGitLab => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                let domain = parsed
                    .strings
                    .get("domain")
                    .map(|s| s.as_str())
                    .unwrap_or("gitlab.com");
                Some(format!("https://{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromGitea | Self::FetchFromForgejo => {
                let domain = parsed.strings.get("domain")?;
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("https://{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromCodeberg => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("https://codeberg.org/{}/{}", owner, repo))
            }
            Self::FetchFromSourcehut => {
                let owner = parsed.strings.get("owner")?;
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
                let owner_with_tilde = if owner.starts_with('~') {
                    owner.clone()
                } else {
                    format!("~{}", owner)
                };
                Some(format!(
                    "https://{}.{}/{}/{}",
                    vc, domain, owner_with_tilde, repo
                ))
            }
            Self::FetchFromBitbucket => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("https://bitbucket.org/{}/{}", owner, repo))
            }
            Self::FetchFromGitiles => parsed.strings.get("url").cloned(),
            Self::FetchFromRepoOrCz => {
                let repo = parsed.strings.get("repo")?;
                Some(format!("https://repo.or.cz/{}.git", repo))
            }
            Self::BuiltinsFetchGit => parsed.strings.get("url").cloned(),
        }
    }

    pub fn uses_tarball(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> bool {
        !self.uses_fetchgit(parsed, has_sparse_checkout) && !self.uses_fetch_submodules(parsed)
    }

    pub fn uses_fetch_submodules(&self, parsed: &ParsedAttrs) -> bool {
        match self {
            Self::FetchGit
            | Self::FetchFromGitHub
            | Self::FetchFromGitLab
            | Self::FetchFromGitea
            | Self::FetchFromForgejo
            | Self::FetchFromCodeberg
            | Self::FetchFromBitbucket
            | Self::FetchFromSourcehut
            | Self::FetchFromGitiles
            | Self::FetchFromRepoOrCz => parsed.bools.get("fetchSubmodules").is_some_and(|&v| v),
            Self::BuiltinsFetchGit => parsed.bools.get("submodules").is_some_and(|&v| v),
            Self::FetchPatch | Self::FetchTarball => false,
        }
    }

    fn uses_fetchgit(&self, parsed: &ParsedAttrs, has_sparse_checkout: bool) -> bool {
        match self {
            Self::FetchGit => true,
            Self::FetchFromGitHub
            | Self::FetchFromGitLab
            | Self::FetchFromGitea
            | Self::FetchFromForgejo
            | Self::FetchFromCodeberg
            | Self::FetchFromBitbucket
            | Self::FetchFromSourcehut
            | Self::FetchFromGitiles
            | Self::FetchFromRepoOrCz => {
                parsed.bools.get("forceFetchGit").is_some_and(|&v| v)
                    || parsed.bools.get("leaveDotGit").is_some_and(|&v| v)
                    || parsed.bools.get("deepClone").is_some_and(|&v| v)
                    || parsed.bools.get("fetchLFS").is_some_and(|&v| v)
                    || parsed.bools.get("fetchSubmodules").is_some_and(|&v| v)
                    || parsed.strings.get("rootDir").is_some_and(|v| !v.is_empty())
                    || has_sparse_checkout
            }
            Self::BuiltinsFetchGit => true,
            Self::FetchPatch | Self::FetchTarball => false,
        }
    }

    pub fn attr_spec(&self) -> &'static [AttrSpec] {
        match self {
            Self::FetchGit => &SPEC_URL_GIT,
            Self::FetchPatch => &SPEC_FETCH_PATCH,
            Self::FetchFromGitHub => &SPEC_GITHUB,
            Self::FetchFromGitLab => &SPEC_GITLAB,
            Self::FetchFromGitea | Self::FetchFromForgejo => &SPEC_GITEA_FORGEJO,
            Self::FetchFromCodeberg => &SPEC_CODEBERG,
            Self::FetchFromSourcehut => &SPEC_SOURCEHUT,
            Self::FetchFromBitbucket => &SPEC_BITBUCKET,
            Self::FetchFromGitiles => &SPEC_GITILES,
            Self::FetchFromRepoOrCz => &SPEC_REPO_OR_CZ,
            Self::BuiltinsFetchGit => &SPEC_BUILTINS_FETCH_GIT,
            Self::FetchTarball => &SPEC_FETCH_TARBALL,
        }
    }

    pub fn operational_keys(&self) -> Vec<&'static str> {
        self.attr_spec().iter().map(|s| s.key).collect()
    }

    pub fn display_target(&self, parsed: &ParsedAttrs) -> Option<String> {
        match self {
            Self::FetchFromGitHub => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("github.com/{}/{}", owner, repo))
            }
            Self::FetchFromGitLab => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                let domain = parsed
                    .strings
                    .get("domain")
                    .map(|s| s.as_str())
                    .unwrap_or("gitlab.com");
                Some(format!("{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromGitea | Self::FetchFromForgejo => {
                let domain = parsed.strings.get("domain")?;
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromCodeberg => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("codeberg.org/{}/{}", owner, repo))
            }
            Self::FetchFromSourcehut => {
                let owner = parsed.strings.get("owner")?;
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
                let owner_with_tilde = if owner.starts_with('~') {
                    owner.clone()
                } else {
                    format!("~{}", owner)
                };
                Some(format!("{}.{}/{}/{}", vc, domain, owner_with_tilde, repo))
            }
            Self::FetchFromBitbucket => {
                let owner = parsed.strings.get("owner")?;
                let repo = parsed.strings.get("repo")?;
                Some(format!("bitbucket.org/{}/{}", owner, repo))
            }
            Self::FetchFromRepoOrCz => {
                let repo = parsed.strings.get("repo")?;
                Some(format!("repo.or.cz/{}.git", repo))
            }
            Self::FetchPatch | Self::FetchTarball => parsed.strings.get("url").and_then(|url| {
                let parsed = parse_source_url(url)?;
                Some(format!("{}/{}", parsed.domain, parsed.project))
            }),
            Self::FetchFromGitiles | Self::FetchGit | Self::BuiltinsFetchGit => {
                parsed.strings.get("url").map(|url| {
                    url.strip_prefix("https://")
                        .or_else(|| url.strip_prefix("http://"))
                        .or_else(|| url.strip_prefix("ssh://"))
                        .or_else(|| url.strip_prefix("git://"))
                        .unwrap_or(url)
                        .to_string()
                })
            }
        }
    }
}

// --- Attribute specs per fetcher kind ---

const SPEC_URL_GIT: [AttrSpec; 15] = [
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
        key: "submodules",
        attr_type: AttrType::Bool,
    },
    AttrSpec {
        key: "forceFetchGit",
        attr_type: AttrType::Bool,
    },
];

const SPEC_GITHUB: [AttrSpec; 16] = [
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

const SPEC_GITLAB: [AttrSpec; 16] = [
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

const SPEC_GITEA_FORGEJO: [AttrSpec; 16] = [
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

const SPEC_CODEBERG: [AttrSpec; 15] = [
    AttrSpec {
        key: "owner",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "repo",
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

const SPEC_SOURCEHUT: [AttrSpec; 17] = [
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

const SPEC_BITBUCKET: [AttrSpec; 15] = [
    AttrSpec {
        key: "owner",
        attr_type: AttrType::String,
    },
    AttrSpec {
        key: "repo",
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

const SPEC_GITILES: [AttrSpec; 14] = [
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

const SPEC_REPO_OR_CZ: [AttrSpec; 14] = [
    AttrSpec {
        key: "repo",
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

const SPEC_BUILTINS_FETCH_GIT: [AttrSpec; 6] = [
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

const SPEC_FETCH_PATCH: [AttrSpec; 28] = [
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

const SPEC_FETCH_TARBALL: [AttrSpec; 5] = [
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_kind_from_name() {
        assert_eq!(
            FetcherKind::from_name("fetchFromGitHub"),
            Some(FetcherKind::FetchFromGitHub)
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
            Some(FetcherKind::FetchFromGitHub)
        );
        assert_eq!(
            FetcherKind::from_name("pkgs.fetchgit"),
            Some(FetcherKind::FetchGit)
        );
        assert_eq!(
            FetcherKind::from_name("lib.fetchFromGitLab"),
            Some(FetcherKind::FetchFromGitLab)
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
        let url = FetcherKind::FetchFromGitHub.git_url(&params).unwrap();
        assert_eq!(url, "https://github.com/NixOS/nixpkgs");
    }

    #[test]
    fn test_fetcher_git_url_gitlab() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("owner".to_string(), "foo".to_string());
        params.strings.insert("repo".to_string(), "bar".to_string());
        let url = FetcherKind::FetchFromGitLab.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromGitea.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
        assert_eq!(url, "https://hg.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_repo_or_cz() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("repo".to_string(), "testrepo".to_string());
        let url = FetcherKind::FetchFromRepoOrCz.git_url(&params).unwrap();
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
        let url = FetcherKind::FetchFromGitiles.git_url(&params);
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
        let url = FetcherKind::FetchFromBitbucket.git_url(&params).unwrap();
        assert_eq!(url, "https://bitbucket.org/testowner/testrepo");
    }

    #[test]
    fn test_uses_fetch_submodules_true_fetchsubmodules() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(FetcherKind::FetchGit.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitLab.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitea.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromForgejo.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromBitbucket.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromSourcehut.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitiles.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_false() {
        let params = ParsedAttrs::default();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_builtins_fetch_git() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("submodules".to_string(), true);
        assert!(FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));

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
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitea.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromForgejo.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromBitbucket.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromSourcehut.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitiles.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_leave_dot_git() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("leaveDotGit".to_string(), true);
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_deep_clone() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), true);
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_lfs() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchLFS".to_string(), true);
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_submodules() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_root_dir() {
        let mut params = ParsedAttrs::default();
        params
            .strings
            .insert("rootDir".to_string(), "/some/path".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.strings.insert("rootDir".to_string(), String::new());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_sparse_checkout() {
        let params = ParsedAttrs::default();
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, true));
        assert!(FetcherKind::FetchFromGitLab.uses_fetchgit(&params, true));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, true));

        let params = ParsedAttrs::default();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_false_when_no_trigger_params() {
        let params = ParsedAttrs::default();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("leaveDotGit".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchLFS".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), false);
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_tarball_supported_fetchers() {
        let params = ParsedAttrs::default();
        assert!(FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitLab.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitea.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromForgejo.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromCodeberg.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromSourcehut.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromBitbucket.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitiles.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchGit.uses_tarball(&params, false));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_fetchgit_flags() {
        let mut params = ParsedAttrs::default();
        params.bools.insert("forceFetchGit".to_string(), true);
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_tarball(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("fetchSubmodules".to_string(), true);
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));

        let mut params = ParsedAttrs::default();
        params.bools.insert("deepClone".to_string(), true);
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_sparse_checkout() {
        let params = ParsedAttrs::default();
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, true));
        assert!(!FetcherKind::FetchFromGitLab.uses_tarball(&params, true));
        assert!(!FetcherKind::FetchFromCodeberg.uses_tarball(&params, true));
    }

    #[test]
    fn test_hash_strategy() {
        let params = ParsedAttrs::default();
        assert_eq!(
            FetcherKind::FetchFromGitHub.hash_strategy(&params, false),
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
            FetcherKind::FetchFromGitHub.hash_strategy(&params, false),
            HashStrategy::Git
        );
    }
}
