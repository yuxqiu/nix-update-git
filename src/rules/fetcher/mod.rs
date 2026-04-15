use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, TextRange};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, VersionDetector};

use kind::{FetcherKind, HashStrategy};

pub mod git_fetch;
pub mod kind;
pub mod tarball;

pub(crate) fn is_commit_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn version_ref_key_and_value(
    kind: FetcherKind,
    params: &HashMap<String, String>,
) -> Option<(&'static str, String)> {
    if let Some(tag) = params.get("tag") {
        return Some(("tag", tag.clone()));
    }
    if let Some(rev) = params.get("rev") {
        if is_commit_hash(rev) || !VersionDetector::is_version(rev) {
            return None;
        }
        return Some(("rev", rev.clone()));
    }
    if let Some(ref_val) = params.get("ref")
        && kind == FetcherKind::BuiltinsFetchGit
    {
        if is_commit_hash(ref_val) || !VersionDetector::is_version(ref_val) {
            return None;
        }
        return Some(("ref", ref_val.clone()));
    }
    None
}

pub(crate) fn preferred_ref_key(params: &HashMap<String, String>) -> Option<&'static str> {
    if params.contains_key("tag") {
        Some("tag")
    } else if params.contains_key("rev") {
        Some("rev")
    } else if params.contains_key("ref") {
        Some("ref")
    } else {
        None
    }
}

/// Resolve a ref value to a revision suitable for prefetching.
///
/// Currently returns the ref as-is for non-empty values (commit hashes
/// and symbolic refs like tags are passed through unchanged). The
/// `git_url` parameter is reserved for future use where symbolic refs
/// may be resolved to commit SHAs via `git ls-remote`.
pub(crate) fn resolve_ref_for_prefetch(_git_url: &str, ref_value: &str) -> Option<String> {
    if ref_value.is_empty() {
        return None;
    }
    Some(ref_value.to_string())
}

struct FetcherCall {
    kind: FetcherKind,
    params: HashMap<String, String>,
    source_ranges: HashMap<String, TextRange>,
    pinned: bool,
    follow_branch: Option<String>,
    sparse_checkout: Vec<String>,
}

#[derive(Default)]
pub struct FetcherRule;

impl FetcherRule {
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
                    if let Some(content) = value.pure_string_content() {
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
                            && let Some(content) = item.pure_string_content()
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
        let Some((version_key, current_version)) =
            version_ref_key_and_value(call.kind, &call.params)
        else {
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

            Ok(Some(latest))
        } else {
            Ok(None)
        }
    }

    fn resolve_rev(call: &FetcherCall, git_url: &str) -> Option<String> {
        let key = preferred_ref_key(&call.params)?;
        let ref_value = call.params.get(key)?;
        resolve_ref_for_prefetch(git_url, ref_value)
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
        let rule = super::FetcherRule;
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
        let rule = super::FetcherRule;
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
        let rule = super::FetcherRule;
        assert!(rule.matches(&fetcher_node));
    }

    #[test]
    fn test_resolve_ref_for_prefetch_keeps_symbolic_ref() {
        let result = super::resolve_ref_for_prefetch("https://example.com/repo", "v1.2.3");
        assert_eq!(result.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn test_resolve_ref_for_prefetch_keeps_commit_hash() {
        let rev = "4f56fd184ef6020626492a6f954a486d54f8b7ba";
        let result = super::resolve_ref_for_prefetch("https://example.com/repo", rev);
        assert_eq!(result.as_deref(), Some(rev));
    }
}
