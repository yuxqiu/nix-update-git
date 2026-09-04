use crate::parser::{NixNode, ParsedAttrs};
use crate::rules::traits::{CheckResult, UpdateRule};
use crate::utils::VersionDetector;

use kind::FetcherKind;

mod dispatch;
mod extract;
mod follow;
pub mod git_fetch;
pub(crate) mod hashing;
mod interpolate;
pub mod kind;
pub mod source_url;
pub mod tarball;

pub(crate) use interpolate::{InterpolationSpec, parse_fetcher_attrset};

pub(crate) fn is_commit_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Used only by `dispatch::check_fetcher_call`/`handle_version_update`,
/// unlike its neighbors above, which `derivation/extract.rs` and
/// `derivation/resolve.rs` also call.
fn version_ref_key_and_value(
    kind: FetcherKind,
    parsed: &ParsedAttrs,
) -> Option<(&'static str, String)> {
    if let Some(tag) = parsed.strings.get("tag") {
        return Some(("tag", tag.clone()));
    }
    if let Some(rev) = parsed.strings.get("rev") {
        if is_commit_hash(rev) || !VersionDetector::is_version(rev) {
            return None;
        }
        return Some(("rev", rev.clone()));
    }
    if let Some(ref_val) = parsed.strings.get("ref")
        && kind == FetcherKind::BuiltinsFetchGit
    {
        if is_commit_hash(ref_val) || !VersionDetector::is_version(ref_val) {
            return None;
        }
        return Some(("ref", ref_val.clone()));
    }
    None
}

pub(crate) fn preferred_ref_key(parsed: &ParsedAttrs) -> Option<&'static str> {
    if parsed.strings.contains_key("tag") {
        Some("tag")
    } else if parsed.strings.contains_key("rev") {
        Some("rev")
    } else if parsed.strings.contains_key("ref") {
        Some("ref")
    } else {
        None
    }
}

pub(crate) fn resolve_ref_for_prefetch(_git_url: &str, ref_value: &str) -> Option<String> {
    if ref_value.is_empty() {
        return None;
    }
    Some(ref_value.to_string())
}

pub struct FetcherRule;

impl UpdateRule for FetcherRule {
    fn name(&self) -> &'static str {
        "fetcher"
    }

    fn matches(&self, node: &NixNode) -> bool {
        if node.kind() != rnix::SyntaxKind::NODE_APPLY {
            return false;
        }
        if extract::is_src_of_owned_call(node) {
            return false;
        }
        true
    }

    fn check(&self, node: &NixNode) -> CheckResult {
        let Some(call) = extract::try_extract_call(node) else {
            return CheckResult::empty();
        };

        let target = call.kind().display_target(call.parsed());

        let mut result = match call.kind() {
            FetcherKind::FetchPatch => dispatch::check_fetchpatch_call(&call),
            FetcherKind::FetchTarball => dispatch::check_fetchtarball_call(&call),
            FetcherKind::BuiltinsFetchGit | FetcherKind::FetchGit | FetcherKind::Forge(_) => {
                dispatch::check_fetcher_call(&call)
            }
        };

        for group in &mut result.groups {
            group.target.clone_from(&target);
        }

        result
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
    fn test_matches_excludes_src_in_lambda_wrapped_mk_derivation() {
        let content = r#"
stdenv.mkDerivation (finalAttrs: {
  version = "v1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
})
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let rule = super::FetcherRule;
        assert!(!rule.matches(&fetcher_node));
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
