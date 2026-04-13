use crate::parser::{NixNode, TextRange};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, NixPrefetcher, TarballHasher, VersionDetector};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetcherKind {
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
    FetchFrom9Front,
    BuiltinsFetchGit,
}

impl FetcherKind {
    fn from_name(name: &str) -> Option<Self> {
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
            "fetchFrom9Front" | "fetchFrom9front" => Some(Self::FetchFrom9Front),
            "fetchGit" => Some(Self::BuiltinsFetchGit),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
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
            Self::FetchFrom9Front => "fetchFrom9Front",
            Self::BuiltinsFetchGit => "builtins.fetchGit",
        }
    }

    fn needs_hash(&self) -> bool {
        !matches!(self, Self::BuiltinsFetchGit)
    }

    fn uses_tarball(&self, params: &HashMap<String, String>) -> bool {
        matches!(
            self,
            Self::FetchFromGitHub | Self::FetchFromGitLab | Self::FetchFromCodeberg
        ) && !self.uses_fetch_submodules(params)
    }

    fn git_url(&self, params: &HashMap<String, String>) -> Option<String> {
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
            Self::FetchFrom9Front => {
                let owner = params.get("owner")?;
                let repo = params.get("repo")?;
                let domain = params
                    .get("domain")
                    .map(|s| s.as_str())
                    .unwrap_or("git.9front.org");
                Some(format!("https://{}/{}/{}", domain, owner, repo))
            }
            Self::BuiltinsFetchGit => params.get("url").cloned(),
        }
    }

    fn uses_fetch_submodules(&self, params: &HashMap<String, String>) -> bool {
        match self {
            Self::FetchFromGitHub
            | Self::FetchFromGitLab
            | Self::FetchFromGitea
            | Self::FetchFromForgejo
            | Self::FetchFromCodeberg
            | Self::FetchFromBitbucket => {
                params.get("fetchSubmodules").is_some_and(|v| v == "true")
                    || params.get("forceFetchGit").is_some_and(|v| v == "true")
            }
            _ => false,
        }
    }
}

struct FetcherCall {
    kind: FetcherKind,
    params: HashMap<String, String>,
    source_ranges: HashMap<String, TextRange>,
    pinned: bool,
    follow_branch: Option<String>,
}

pub struct FetcherRule {
    no_prefetch: bool,
}

impl FetcherRule {
    pub fn new() -> Self {
        Self { no_prefetch: false }
    }

    pub fn with_no_prefetch(mut self, no_prefetch: bool) -> Self {
        self.no_prefetch = no_prefetch;
        self
    }

    fn extract_fetcher_calls(root: &NixNode) -> Vec<FetcherCall> {
        let mut calls = Vec::new();
        for node in root.traverse() {
            if node.kind() != rnix::SyntaxKind::NODE_APPLY {
                continue;
            }

            let func_name = match node.apply_function_name() {
                Some(name) => name,
                None => continue,
            };

            let kind = match FetcherKind::from_name(&func_name) {
                Some(k) => k,
                None => continue,
            };

            let arg = match node.apply_argument() {
                Some(arg) => arg,
                None => continue,
            };

            if arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
                continue;
            }

            let mut params = HashMap::new();
            let mut source_ranges = HashMap::new();

            for child in arg.children() {
                if child.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
                    continue;
                }
                let segments = child.attrpath_segments();
                if segments.len() != 1 {
                    continue;
                }
                let key = segments[0].clone();

                if let Some(value) = child.attr_value() {
                    if value.kind() == rnix::SyntaxKind::NODE_STRING {
                        if let Some(content) = value.string_content() {
                            params.insert(key.clone(), content);
                            source_ranges.insert(key, value.text_range());
                        }
                    } else if value.kind() == rnix::SyntaxKind::NODE_IDENT {
                        let text = value.text();
                        let trimmed = text.trim();
                        if trimmed == "true" || trimmed == "false" {
                            params.insert(key.clone(), trimmed.to_string());
                        }
                    }
                }
            }

            let pinned = arg.has_pin_comment() || node.has_pin_comment();
            let follow_branch = arg
                .follow_branch_comment()
                .or_else(|| node.follow_branch_comment());

            calls.push(FetcherCall {
                kind,
                params,
                source_ranges,
                pinned,
                follow_branch,
            });
        }
        calls
    }

    fn check_fetcher_call(&self, call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        if call.pinned {
            return Ok(None);
        }

        let git_url = match call.kind.git_url(&call.params) {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();

        if let Some(branch) = &call.follow_branch {
            self.handle_branch_following(call, &git_url, branch, &mut updates)?;
        } else {
            self.handle_version_update(call, &git_url, &mut updates)?;
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }

    /// Resolve the latest commit on `branch` and produce an update for it.
    ///
    /// Disambiguates which attribute key to update based on what the call
    /// already contains:
    ///
    /// - If `rev` key exists in the call → update `rev`.
    /// - If `ref` key exists and the call is `builtins.fetchGit` → update `ref`
    ///   (since `ref` is the standard attribute for branch names in
    ///   `builtins.fetchGit`).
    /// - Otherwise → update `rev` (the default for all other fetcher kinds).
    fn handle_branch_following(
        &self,
        call: &FetcherCall,
        git_url: &str,
        branch: &str,
        updates: &mut Vec<Update>,
    ) -> Result<()> {
        let new_sha = match GitFetcher::get_latest_commit(git_url, branch)? {
            Some(sha) => sha,
            None => {
                eprintln!(
                    "Warning: could not find branch '{}' for {}",
                    branch, git_url
                );
                return Ok(());
            }
        };

        let current_ref = call.params.get("rev").or_else(|| call.params.get("ref"));

        if let Some(current) = current_ref
            && current == &new_sha
        {
            return Ok(());
        }

        let ref_key = if call.params.contains_key("rev") {
            "rev"
        } else if call.kind == FetcherKind::BuiltinsFetchGit {
            "ref"
        } else {
            "rev"
        };

        if let Some(range) = call.source_ranges.get(ref_key) {
            updates.push(Update::new(
                format!("{}.rev", call.kind.name()),
                format!("\"{}\"", new_sha),
                *range,
            ));

            if call.kind.needs_hash() {
                Self::try_prefetch_hash(self.no_prefetch, call, &new_sha, updates);
            }
        }

        Ok(())
    }

    /// Find the latest version tag on `git_url` and produce an update if it's
    /// newer than the current value.
    ///
    /// Attribute selection follows this priority:
    ///
    /// 1. `tag` — if present, update the `tag` attribute (used by
    ///    `fetchFromGitHub` and similar fetchers that support a separate `tag`
    ///    parameter).
    /// 2. `rev` — if present and looks like a version (not a commit hash),
    ///    update `rev`.
    /// 3. `ref` — only for `builtins.fetchGit`, which uses `ref` for version
    ///    tags instead of `rev`. Skip for other fetcher kinds.
    fn handle_version_update(
        &self,
        call: &FetcherCall,
        git_url: &str,
        updates: &mut Vec<Update>,
    ) -> Result<()> {
        let (version_key, current_version) = if let Some(tag) = call.params.get("tag") {
            ("tag", tag.clone())
        } else if let Some(rev) = call.params.get("rev") {
            if Self::is_commit_hash(rev) {
                return Ok(());
            }
            if !VersionDetector::is_version(rev) {
                return Ok(());
            }
            ("rev", rev.clone())
        } else if let Some(ref_val) = call.params.get("ref") {
            if call.kind == FetcherKind::BuiltinsFetchGit {
                if Self::is_commit_hash(ref_val) {
                    return Ok(());
                }
                if !VersionDetector::is_version(ref_val) {
                    return Ok(());
                }
                ("ref", ref_val.clone())
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        let latest = match GitFetcher::get_latest_tag_matching(git_url, Some(&current_version))? {
            Some(tag) => tag,
            None => return Ok(()),
        };

        if VersionDetector::compare(&current_version, &latest) != std::cmp::Ordering::Less {
            return Ok(());
        }

        if let Some(range) = call.source_ranges.get(version_key) {
            updates.push(Update::new(
                format!("{}.{}", call.kind.name(), version_key),
                format!("\"{}\"", latest),
                *range,
            ));

            if call.kind.needs_hash() {
                let rev_for_prefetch = GitFetcher::resolve_ref_to_sha(git_url, &latest)
                    .ok()
                    .flatten();
                let prefetch_rev = rev_for_prefetch.as_deref().unwrap_or(&latest);
                Self::try_prefetch_hash(self.no_prefetch, call, prefetch_rev, updates);
            }
        }

        Ok(())
    }

    fn is_commit_hash(s: &str) -> bool {
        s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    fn try_prefetch_hash(
        no_prefetch: bool,
        call: &FetcherCall,
        rev: &str,
        updates: &mut Vec<Update>,
    ) {
        if no_prefetch {
            return;
        }

        let has_sri_hash = call
            .params
            .get("hash")
            .is_some_and(|h| h.starts_with("sha256-") || h.starts_with("sha512-"));
        let has_nix32_hash = call.params.contains_key("sha256");

        if !has_sri_hash && !has_nix32_hash {
            return;
        }

        let result = Self::compute_hash(call, rev);

        match result {
            Ok(nar_hash) => {
                if has_sri_hash && let Some(range) = call.source_ranges.get("hash") {
                    updates.push(Update::new(
                        format!("{}.hash", call.kind.name()),
                        format!("\"{}\"", nar_hash.sri),
                        *range,
                    ));
                }
                if has_nix32_hash && let Some(range) = call.source_ranges.get("sha256") {
                    updates.push(Update::new(
                        format!("{}.sha256", call.kind.name()),
                        format!("\"{}\"", nar_hash.nix32),
                        *range,
                    ));
                }
            }
            Err(e) => {
                let git_url = call.kind.git_url(&call.params).unwrap_or_default();
                eprintln!(
                    "Warning: could not prefetch hash for {} @ {}: {}",
                    git_url, rev, e
                );
            }
        }
    }

    fn compute_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        if call.kind.uses_tarball(&call.params) {
            Self::compute_tarball_hash(call, rev)
        } else {
            Self::compute_git_hash(call, rev)
        }
    }

    fn compute_tarball_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        match call.kind {
            FetcherKind::FetchFromGitHub => {
                let owner = call.params.get("owner").map(|s| s.as_str()).unwrap_or("");
                let repo = call.params.get("repo").map(|s| s.as_str()).unwrap_or("");
                let github_base = call
                    .params
                    .get("githubBase")
                    .map(|s| s.as_str())
                    .unwrap_or("github.com");
                let url = format!(
                    "https://{}/{}/{}/archive/{}.tar.gz",
                    github_base, owner, repo, rev
                );
                TarballHasher::hash_tarball_url(&url)
            }
            FetcherKind::FetchFromGitLab => {
                let domain = call
                    .params
                    .get("domain")
                    .map(|s| s.as_str())
                    .unwrap_or("gitlab.com");
                let owner = call.params.get("owner").map(|s| s.as_str()).unwrap_or("");
                let repo = call.params.get("repo").map(|s| s.as_str()).unwrap_or("");
                let url = format!(
                    "https://{}/{}/{}/-/archive/{}/{}-{}.tar.gz",
                    domain, owner, repo, rev, repo, rev
                );
                TarballHasher::hash_tarball_url(&url)
            }
            FetcherKind::FetchFromCodeberg => {
                let owner = call.params.get("owner").map(|s| s.as_str()).unwrap_or("");
                let repo = call.params.get("repo").map(|s| s.as_str()).unwrap_or("");
                let url = format!(
                    "https://codeberg.org/{}/{}/archive/{}.tar.gz",
                    owner, repo, rev
                );
                TarballHasher::hash_tarball_url(&url)
            }
            _ => anyhow::bail!("Unsupported fetcher for tarball hashing"),
        }
    }

    fn compute_git_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        let git_url = match call.kind.git_url(&call.params) {
            Some(url) => url,
            None => anyhow::bail!("No git URL available"),
        };
        let use_submodules = call.kind.uses_fetch_submodules(&call.params);
        let prefetch = if use_submodules {
            NixPrefetcher::prefetch_git_with_submodules(&git_url, rev)?
        } else {
            NixPrefetcher::prefetch_git(&git_url, rev)?
        };
        Ok(NarHash {
            sri: prefetch.sri_hash,
            nix32: prefetch.sha256_nix,
            hex: String::new(),
        })
    }
}

impl Default for FetcherRule {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateRule for FetcherRule {
    fn name(&self) -> &str {
        "fetcher"
    }

    fn matches(&self, node: &NixNode) -> bool {
        node.kind() == rnix::SyntaxKind::NODE_ROOT || node.kind() == rnix::SyntaxKind::NODE_ATTR_SET
    }

    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>> {
        let mut all_updates = Vec::new();

        for call in Self::extract_fetcher_calls(node) {
            if let Some(updates) = self.check_fetcher_call(&call)? {
                all_updates.extend(updates);
            }
        }

        if all_updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_updates))
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
            FetcherKind::from_name("fetchFrom9front"),
            Some(FetcherKind::FetchFrom9Front)
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
    fn test_is_commit_hash() {
        assert!(FetcherRule::is_commit_hash(
            "abc123def456abc123def456abc123def456abc1"
        ));
        assert!(!FetcherRule::is_commit_hash("v1.0.0"));
        assert!(!FetcherRule::is_commit_hash("main"));
        assert!(!FetcherRule::is_commit_hash("short"));
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
    fn test_uses_fetch_submodules_true_fetchsubmodules() {
        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitLab.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitea.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromForgejo.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromCodeberg.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromBitbucket.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_true_forcefetchgit() {
        let mut params = HashMap::new();
        params.insert("forceFetchGit".to_string(), "true".to_string());
        assert!(FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
        assert!(FetcherKind::FetchFromGitLab.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_false() {
        let params = HashMap::new();
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));

        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "false".to_string());
        assert!(!FetcherKind::FetchFromGitHub.uses_fetch_submodules(&params));
    }

    #[test]
    fn test_uses_fetch_submodules_other_fetchers() {
        let mut params = HashMap::new();
        params.insert("fetchSubmodules".to_string(), "true".to_string());
        assert!(!FetcherKind::FetchGit.uses_fetch_submodules(&params));
        assert!(!FetcherKind::BuiltinsFetchGit.uses_fetch_submodules(&params));
        assert!(!FetcherKind::FetchFromSavannah.uses_fetch_submodules(&params));
    }
}
