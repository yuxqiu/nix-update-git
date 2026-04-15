use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, TextRange};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, VersionDetector};

use kind::{FetcherKind, HashStrategy};

pub mod git_fetch;
pub mod kind;
pub mod tarball;

struct FetcherCall {
    kind: FetcherKind,
    params: HashMap<String, String>,
    source_ranges: HashMap<String, TextRange>,
    pinned: bool,
    follow_branch: Option<String>,
    sparse_checkout: Vec<String>,
}

pub struct FetcherRule;

impl FetcherRule {
    pub fn new() -> Self {
        Self
    }

    fn try_extract_call(node: &NixNode) -> Option<FetcherCall> {
        let func_name = node.apply_function_name()?;
        let kind = FetcherKind::from_name(&func_name)?;
        let arg = node.apply_argument()?;

        if arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return None;
        }

        let mut params = HashMap::new();
        let mut source_ranges = HashMap::new();
        let mut sparse_checkout = Vec::new();

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
                } else if key == "sparseCheckout" && value.kind() == rnix::SyntaxKind::NODE_LIST {
                    for item in value.children() {
                        if item.kind() == rnix::SyntaxKind::NODE_STRING
                            && let Some(content) = item.string_content()
                        {
                            sparse_checkout.push(content);
                        }
                    }
                }
            }
        }

        let pinned = arg.has_pin_comment() || node.has_pin_comment();
        let follow_branch = arg
            .follow_branch_comment()
            .or_else(|| node.follow_branch_comment());

        Some(FetcherCall {
            kind,
            params,
            source_ranges,
            pinned,
            follow_branch,
            sparse_checkout,
        })
    }

    fn check_fetcher_call(&self, call: &FetcherCall) -> Result<Option<Vec<Update>>> {
        let git_url = match call.kind.git_url(&call.params) {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut version_updated_rev: Option<String> = None;

        // Case 1: not pinned -> check version update
        if !call.pinned {
            if let Some(branch) = &call.follow_branch {
                version_updated_rev =
                    self.handle_branch_following(call, &git_url, branch, &mut updates)?;
            } else {
                version_updated_rev = self.handle_version_update(call, &git_url, &mut updates)?;
            }
        }

        // Case 2: update hash if needed
        if call.kind.needs_hash() {
            if let Some(rev) = &version_updated_rev {
                Self::try_prefetch_hash(call, rev, &mut updates);
            } else {
                Self::try_prefetch_empty_hash(call, &git_url, &mut updates);
            }
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }

    fn handle_branch_following(
        &self,
        call: &FetcherCall,
        git_url: &str,
        branch: &str,
        updates: &mut Vec<Update>,
    ) -> Result<Option<String>> {
        let new_sha = match GitFetcher::get_latest_commit(git_url, branch)? {
            Some(sha) => sha,
            None => {
                eprintln!(
                    "Warning: could not find branch '{}' for {}",
                    branch, git_url
                );
                return Ok(None);
            }
        };

        let current_ref = call.params.get("rev").or_else(|| call.params.get("ref"));

        if let Some(current) = current_ref
            && current == &new_sha
        {
            return Ok(None);
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

            Ok(Some(new_sha))
        } else {
            Ok(None)
        }
    }

    fn handle_version_update(
        &self,
        call: &FetcherCall,
        git_url: &str,
        updates: &mut Vec<Update>,
    ) -> Result<Option<String>> {
        let (version_key, current_version) = if let Some(tag) = call.params.get("tag") {
            ("tag", tag.clone())
        } else if let Some(rev) = call.params.get("rev") {
            if Self::is_commit_hash(rev) {
                return Ok(None);
            }
            if !VersionDetector::is_version(rev) {
                return Ok(None);
            }
            ("rev", rev.clone())
        } else if let Some(ref_val) = call.params.get("ref") {
            if call.kind == FetcherKind::BuiltinsFetchGit {
                if Self::is_commit_hash(ref_val) {
                    return Ok(None);
                }
                if !VersionDetector::is_version(ref_val) {
                    return Ok(None);
                }
                ("ref", ref_val.clone())
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        };

        let latest = match GitFetcher::get_latest_tag_matching(git_url, Some(&current_version))? {
            Some(tag) => tag,
            None => return Ok(None),
        };

        if VersionDetector::compare(&current_version, &latest) != std::cmp::Ordering::Less {
            return Ok(None);
        }

        if let Some(range) = call.source_ranges.get(version_key) {
            updates.push(Update::new(
                format!("{}.{}", call.kind.name(), version_key),
                format!("\"{}\"", latest),
                *range,
            ));

            let rev_for_prefetch = GitFetcher::resolve_ref_to_sha(git_url, &latest)
                .ok()
                .flatten();
            let prefetch_rev = rev_for_prefetch.as_deref().unwrap_or(&latest);
            Ok(Some(prefetch_rev.to_string()))
        } else {
            Ok(None)
        }
    }

    fn is_commit_hash(s: &str) -> bool {
        s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    fn resolve_rev(call: &FetcherCall, git_url: &str) -> Option<String> {
        let rev = if let Some(tag) = call.params.get("tag") {
            tag.clone()
        } else if let Some(rev) = call.params.get("rev") {
            rev.clone()
        } else if let Some(ref_val) = call.params.get("ref") {
            ref_val.clone()
        } else {
            return None;
        };

        if Self::is_commit_hash(&rev) {
            Some(rev)
        } else {
            GitFetcher::resolve_ref_to_sha(git_url, &rev).ok().flatten()
        }
    }

    fn try_prefetch_hash(call: &FetcherCall, rev: &str, updates: &mut Vec<Update>) {
        if !call.source_ranges.contains_key("hash") && !call.source_ranges.contains_key("sha256") {
            return;
        }

        let result = Self::compute_hash(call, rev);

        match result {
            Ok(nar_hash) => {
                if let Some(range) = call.source_ranges.get("hash") {
                    updates.push(Update::new(
                        format!("{}.hash", call.kind.name()),
                        format!("\"{}\"", nar_hash.sri),
                        *range,
                    ));
                }
                if let Some(range) = call.source_ranges.get("sha256") {
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

    fn try_prefetch_empty_hash(call: &FetcherCall, git_url: &str, updates: &mut Vec<Update>) {
        let has_empty_hash = call.params.get("hash").is_some_and(|h| h.is_empty())
            || call.params.get("sha256").is_some_and(|h| h.is_empty());

        if !has_empty_hash {
            return;
        }

        if let Some(rev) = Self::resolve_rev(call, git_url) {
            Self::try_prefetch_hash(call, &rev, updates);
        }
    }

    fn compute_hash(call: &FetcherCall, rev: &str) -> Result<NarHash> {
        let has_sparse_checkout = !call.sparse_checkout.is_empty();
        match call.kind.hash_strategy(&call.params, has_sparse_checkout) {
            HashStrategy::Tarball => tarball::compute_hash(&call.kind, &call.params, rev),
            HashStrategy::Git => {
                git_fetch::compute_hash(&call.kind, &call.params, rev, &call.sparse_checkout)
            }
            HashStrategy::None => anyhow::bail!("No hash needed for this fetcher"),
        }
    }

    fn is_src_of_active_mk_derivation(node: &NixNode) -> bool {
        let mut current = match node.parent() {
            Some(p) => p,
            None => return false,
        };

        while current.kind() == rnix::SyntaxKind::NODE_PAREN {
            current = match current.parent() {
                Some(p) => p,
                None => return false,
            };
        }

        if current.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            return false;
        }
        let segments = current.attrpath_segments();
        if segments.len() != 1 || segments[0] != "src" {
            return false;
        }

        let attr_set = match current.parent() {
            Some(p) => p,
            None => return false,
        };
        if attr_set.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return false;
        }

        let mk_derivation_apply = match attr_set.parent() {
            Some(p) => p,
            None => return false,
        };
        if mk_derivation_apply.kind() != rnix::SyntaxKind::NODE_APPLY {
            return false;
        }

        let func_name = match mk_derivation_apply.apply_function_name() {
            Some(name) => name,
            None => return false,
        };
        let short_name = func_name.rsplit('.').next().unwrap_or(&func_name);
        if short_name != "mkDerivation" {
            return false;
        }

        true
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
        if node.kind() != rnix::SyntaxKind::NODE_APPLY {
            return false;
        }
        if Self::is_src_of_active_mk_derivation(node) {
            return false;
        }
        true
    }

    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>> {
        let call = match Self::try_extract_call(node) {
            Some(call) => call,
            None => return Ok(None),
        };
        self.check_fetcher_call(&call)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::NixFile;
    use crate::rules::traits::UpdateRule;

    fn parse_root(content: &str) -> crate::parser::NixNode {
        NixFile::parse(content).unwrap().root_node()
    }

    fn find_fetcher_apply(
        root: &crate::parser::NixNode,
        name: &str,
    ) -> Option<crate::parser::NixNode> {
        root.traverse().find(|node| {
            node.kind() == rnix::SyntaxKind::NODE_APPLY
                && node.apply_function_name().as_deref() == Some(name)
        })
    }

    #[test]
    fn test_is_src_of_mk_derivation_returns_true() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_standalone_fetcher_returns_false() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_pkgs_dot_stdenv_dot_mk_derivation_returns_true() {
        let content = r#"
pkgs.stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_fetcher_non_src_attr_returns_false() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  patches = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_fetcher_in_non_mk_derivation_returns_false() {
        let content = r#"
someOtherFunc rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(!super::FetcherRule::is_src_of_active_mk_derivation(
            &fetcher_node
        ));
    }

    #[test]
    fn test_matches_excludes_src_in_mk_derivation() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule::new();
        assert!(!rule.matches(&fetcher_node));
    }

    #[test]
    fn test_matches_allows_standalone_fetcher() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule::new();
        assert!(rule.matches(&fetcher_node));
    }

    #[test]
    fn test_matches_allows_patches_in_mk_derivation() {
        let content = r#"
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "v1.0.0";
  patches = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule::new();
        assert!(rule.matches(&fetcher_node));
    }
}
