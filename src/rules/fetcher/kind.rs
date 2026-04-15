use std::collections::HashMap;

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
    FetchFromSavannah,
    FetchFromRepoOrCz,
    BuiltinsFetchGit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashStrategy {
    Tarball,
    Git,
    None,
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
            "fetchFromSavannah" | "fetchFromSavannahGNU" | "fetchFromSavannahNonGNU" => {
                Some(Self::FetchFromSavannah)
            }
            "fetchFromRepoOrCz" => Some(Self::FetchFromRepoOrCz),
            "fetchGit" => Some(Self::BuiltinsFetchGit),
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
            Self::FetchFromSavannah => "fetchFromSavannah",
            Self::FetchFromRepoOrCz => "fetchFromRepoOrCz",
            Self::BuiltinsFetchGit => "builtins.fetchGit",
        }
    }

    pub fn needs_hash(&self) -> bool {
        !matches!(self, Self::BuiltinsFetchGit)
    }

    pub fn hash_strategy(
        &self,
        params: &HashMap<String, String>,
        has_sparse_checkout: bool,
    ) -> HashStrategy {
        if !self.needs_hash() {
            return HashStrategy::None;
        }
        if self.uses_tarball(params, has_sparse_checkout) {
            HashStrategy::Tarball
        } else {
            HashStrategy::Git
        }
    }

    pub fn git_url(&self, params: &HashMap<String, String>) -> Option<String> {
        match self {
            Self::FetchGit => params.get("url").cloned(),
            Self::FetchFromGitHub => {
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                let base = params
                    .get("githubBase")
                    .map(|s| s.as_str())
                    .unwrap_or("github.com");
                Some(format!("https://{}/{}/{}", base, owner, repo))
            }
            Self::FetchFromGitLab => {
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                let domain = params
                    .get("domain")
                    .map(|s| s.as_str())
                    .unwrap_or("gitlab.com");
                Some(format!("https://{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromGitea | Self::FetchFromForgejo => {
                let domain = params.get("domain")?;
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                Some(format!("https://{}/{}/{}", domain, owner, repo))
            }
            Self::FetchFromCodeberg => {
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                Some(format!("https://codeberg.org/{}/{}", owner, repo))
            }
            Self::FetchFromSourcehut => {
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                let domain = params.get("domain").map(|s| s.as_str()).unwrap_or("sr.ht");
                let vc = params.get("vc").map(|s| s.as_str()).unwrap_or("git");
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
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                Some(format!("https://bitbucket.org/{}/{}", owner, repo))
            }
            Self::FetchFromGitiles => params.get("url").cloned(),
            Self::FetchFromSavannah => {
                let repo = params.get("repo")?;
                Some(format!("https://git.savannah.gnu.org/git/{}.git", repo))
            }
            Self::FetchFromRepoOrCz => {
                let repo = params.get("repo")?;
                Some(format!("https://repo.or.cz/{}.git", repo))
            }
            Self::BuiltinsFetchGit => params.get("url").cloned(),
        }
    }

    pub fn uses_tarball(
        &self,
        params: &HashMap<String, String>,
        has_sparse_checkout: bool,
    ) -> bool {
        !self.uses_fetchgit(params, has_sparse_checkout) && !self.uses_fetch_submodules(params)
    }

    pub fn uses_fetch_submodules(&self, params: &HashMap<String, String>) -> bool {
        match self {
            // https://nixos.org/manual/nixpkgs/stable/#chap-pkgs-fetchers
            Self::FetchGit
            | Self::FetchFromGitHub
            | Self::FetchFromGitLab
            | Self::FetchFromGitea
            | Self::FetchFromForgejo
            | Self::FetchFromCodeberg
            | Self::FetchFromBitbucket
            | Self::FetchFromSavannah
            | Self::FetchFromSourcehut
            | Self::FetchFromGitiles
            | Self::FetchFromRepoOrCz => params.get("fetchSubmodules").is_some_and(|v| v == "true"),
            // https://noogle.dev/f/builtins/fetchGit
            Self::BuiltinsFetchGit => params.get("submodules").is_some_and(|v| v == "true"),
        }
    }

    fn uses_fetchgit(&self, params: &HashMap<String, String>, has_sparse_checkout: bool) -> bool {
        match self {
            Self::FetchGit => true,
            Self::FetchFromGitHub
            | Self::FetchFromGitLab
            | Self::FetchFromGitea
            | Self::FetchFromForgejo
            | Self::FetchFromCodeberg
            | Self::FetchFromBitbucket
            | Self::FetchFromSavannah
            | Self::FetchFromSourcehut
            | Self::FetchFromGitiles
            | Self::FetchFromRepoOrCz => {
                params.get("forceFetchGit").is_some_and(|v| v == "true")
                    || params.get("leaveDotGit").is_some_and(|v| v == "true")
                    || params.get("deepClone").is_some_and(|v| v == "true")
                    || params.get("fetchLFS").is_some_and(|v| v == "true")
                    || params.get("fetchSubmodules").is_some_and(|v| v == "true")
                    || params.get("rootDir").is_some_and(|v| !v.is_empty())
                    || has_sparse_checkout
            }
            Self::BuiltinsFetchGit => true,
        }
    }
}

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
            FetcherKind::from_name("fetchFromSavannahGNU"),
            Some(FetcherKind::FetchFromSavannah)
        );
        assert_eq!(
            FetcherKind::from_name("fetchgitPrivate"),
            Some(FetcherKind::FetchGit)
        );
        assert_eq!(FetcherKind::from_name("unknown"), None);
        assert_eq!(FetcherKind::from_name("pkgs.unknown"), None);
    }

    #[test]
    fn test_fetcher_git_url_github() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "NixOS".to_string());
        params.insert("repo".to_string(), "nixpkgs".to_string());
        let url = FetcherKind::FetchFromGitHub.git_url(&params).unwrap();
        assert_eq!(url, "https://github.com/NixOS/nixpkgs");
    }

    #[test]
    fn test_fetcher_git_url_gitlab() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "foo".to_string());
        params.insert("repo".to_string(), "bar".to_string());
        let url = FetcherKind::FetchFromGitLab.git_url(&params).unwrap();
        assert_eq!(url, "https://gitlab.com/foo/bar");
    }

    #[test]
    fn test_fetcher_git_url_gitea() {
        let mut params = HashMap::new();
        params.insert("domain".to_string(), "gitea.example.com".to_string());
        params.insert("owner".to_string(), "foo".to_string());
        params.insert("repo".to_string(), "bar".to_string());
        let url = FetcherKind::FetchFromGitea.git_url(&params).unwrap();
        assert_eq!(url, "https://gitea.example.com/foo/bar");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "~sirhc".to_string());
        params.insert("repo".to_string(), "repo".to_string());
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
        assert_eq!(url, "https://git.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_no_tilde() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "sirhc".to_string());
        params.insert("repo".to_string(), "repo".to_string());
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
        assert_eq!(url, "https://git.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_custom_domain() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "~sirhc".to_string());
        params.insert("repo".to_string(), "repo".to_string());
        params.insert("domain".to_string(), "custom.sr.ht".to_string());
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
        assert_eq!(url, "https://git.custom.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_sourcehut_custom_vc() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "~sirhc".to_string());
        params.insert("repo".to_string(), "repo".to_string());
        params.insert("vc".to_string(), "hg".to_string());
        let url = FetcherKind::FetchFromSourcehut.git_url(&params).unwrap();
        assert_eq!(url, "https://hg.sr.ht/~sirhc/repo");
    }

    #[test]
    fn test_fetcher_git_url_savannah() {
        let mut params = HashMap::new();
        params.insert("repo".to_string(), "emacs/elpa".to_string());
        let url = FetcherKind::FetchFromSavannah.git_url(&params).unwrap();
        assert_eq!(url, "https://git.savannah.gnu.org/git/emacs/elpa.git");
    }

    #[test]
    fn test_fetcher_git_url_repo_or_cz() {
        let mut params = HashMap::new();
        params.insert("repo".to_string(), "testrepo".to_string());
        let url = FetcherKind::FetchFromRepoOrCz.git_url(&params).unwrap();
        assert_eq!(url, "https://repo.or.cz/testrepo.git");
    }

    #[test]
    fn test_fetcher_git_url_builtins_fetch_git() {
        let mut params = HashMap::new();
        params.insert(
            "url".to_string(),
            "https://example.com/repo.git".to_string(),
        );
        let url = FetcherKind::BuiltinsFetchGit.git_url(&params);
        assert_eq!(url, Some("https://example.com/repo.git".to_string()));
    }

    #[test]
    fn test_fetcher_git_url_fetchgit_with_url() {
        let mut params = HashMap::new();
        params.insert(
            "url".to_string(),
            "https://example.com/repo.git".to_string(),
        );
        let url = FetcherKind::FetchGit.git_url(&params);
        assert_eq!(url, Some("https://example.com/repo.git".to_string()));
    }

    #[test]
    fn test_fetcher_git_url_gitiles() {
        let mut params = HashMap::new();
        params.insert(
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
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "testowner".to_string());
        params.insert("repo".to_string(), "testrepo".to_string());
        let url = FetcherKind::FetchFromBitbucket.git_url(&params).unwrap();
        assert_eq!(url, "https://bitbucket.org/testowner/testrepo");
    }

    #[test]
    fn test_uses_fetch_submodules_true_fetchsubmodules() {
        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(FetcherKind::FetchGit.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitLab.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitea.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromForgejo.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromBitbucket.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromSavannah.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromSourcehut.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitiles.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_false() {
        let params = HashMap::new();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));

        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_builtins_fetch_git() {
        let mut params = HashMap::new();
        params.insert("submodules".to_string(), "true".to_string());
        assert!(FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));

        let mut params = HashMap::new();
        params.insert("submodules".to_string(), "false".to_string());
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));

        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetchgit_always_true() {
        let params = HashMap::new();
        assert!(FetcherKind::FetchGit.uses_fetchgit(&params, false));
        assert!(FetcherKind::BuiltinsFetchGit.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_force_fetch_git() {
        let mut params = HashMap::new();
        params.insert("forceFetchGit".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitea.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromForgejo.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromBitbucket.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromSavannah.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromSourcehut.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromGitiles.uses_fetchgit(&params, false));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_leave_dot_git() {
        let mut params = HashMap::new();
        params.insert("leaveDotGit".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_deep_clone() {
        let mut params = HashMap::new();
        params.insert("deepClone".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_lfs() {
        let mut params = HashMap::new();
        params.insert("fetchLFS".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_fetch_submodules() {
        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_root_dir() {
        let mut params = HashMap::new();
        params.insert("rootDir".to_string(), "/some/path".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("rootDir".to_string(), String::new());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_sparse_checkout() {
        let params = HashMap::new();
        assert!(FetcherKind::FetchFromGitHub.uses_fetchgit(&params, true));
        assert!(FetcherKind::FetchFromGitLab.uses_fetchgit(&params, true));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, true));

        let params = HashMap::new();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_fetchgit_false_when_no_trigger_params() {
        let params = HashMap::new();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_fetchgit(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("forceFetchGit".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("leaveDotGit".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("deepClone".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("fetchLFS".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));

        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetchgit(&params, false));
    }

    #[test]
    fn test_uses_tarball_supported_fetchers() {
        let params = HashMap::new();
        assert!(FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitLab.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitea.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromForgejo.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromCodeberg.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromSourcehut.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromBitbucket.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromGitiles.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromSavannah.uses_tarball(&params, false));
        assert!(FetcherKind::FetchFromRepoOrCz.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchGit.uses_tarball(&params, false));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_fetchgit_flags() {
        let mut params = HashMap::new();
        params.insert("forceFetchGit".to_string(), "true".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchFromGitLab.uses_tarball(&params, false));
        assert!(!FetcherKind::FetchFromCodeberg.uses_tarball(&params, false));

        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));

        let mut params = HashMap::new();
        params.insert("deepClone".to_string(), "true".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, false));
    }

    #[test]
    fn test_uses_tarball_disabled_by_sparse_checkout() {
        let params = HashMap::new();
        assert!(!FetcherKind::FetchFromGitHub.uses_tarball(&params, true));
        assert!(!FetcherKind::FetchFromGitLab.uses_tarball(&params, true));
        assert!(!FetcherKind::FetchFromCodeberg.uses_tarball(&params, true));
    }

    #[test]
    fn test_hash_strategy() {
        let params = HashMap::new();
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

        let mut params = HashMap::new();
        params.insert("forceFetchGit".to_string(), "true".to_string());
        assert_eq!(
            FetcherKind::FetchFromGitHub.hash_strategy(&params, false),
            HashStrategy::Git
        );
    }
}
