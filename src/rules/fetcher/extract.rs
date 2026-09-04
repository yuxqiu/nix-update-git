//! AST extraction: turning a `NODE_APPLY` into a `FetcherCall`, and
//! recognizing when a fetcher call is owned by a derivation rule instead
//! (so the fetcher rule should skip it).

use crate::parser::{NixNode, ParsedAttrs};
use crate::rules::derivation::OWNED_FUNC_NAMES;

use super::interpolate::{InterpolationSpec, parse_fetcher_attrset};
use super::kind::FetcherKind;

/// Only ever constructed by `try_extract_call` — fields are private so
/// nothing downstream can assemble one from arbitrary parts and skip the
/// validation `try_extract_call` does (operational-key interpolation
/// checks, pin/follow-comment resolution, ...).
pub(super) struct FetcherCall {
    kind: FetcherKind,
    parsed: ParsedAttrs,
    pinned: bool,
    follow: Option<String>,
}

impl FetcherCall {
    pub(super) fn kind(&self) -> FetcherKind {
        self.kind
    }

    pub(super) fn parsed(&self) -> &ParsedAttrs {
        &self.parsed
    }

    pub(super) fn pinned(&self) -> bool {
        self.pinned
    }

    pub(super) fn follow(&self) -> Option<&str> {
        self.follow.as_deref()
    }
}

pub(super) fn try_extract_call(node: &NixNode) -> Option<FetcherCall> {
    let func_name = node.apply_function_name()?;
    let kind = FetcherKind::from_name(&func_name)?;
    let arg = node.apply_argument()?;

    if arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
        return None;
    }

    let op_keys = kind.operational_keys();
    let attrs = match parse_fetcher_attrset(kind, &arg, &InterpolationSpec::none()) {
        Ok(a) => a,
        Err(_) => return None,
    };

    if attrs
        .interpolated_unresolved
        .iter()
        .any(|k| op_keys.contains(&k.as_str()))
    {
        return None;
    }

    let pinned = arg.has_pin_comment() || node.has_pin_comment();
    let follow = arg.follow_comment().or_else(|| node.follow_comment());

    Some(FetcherCall {
        kind,
        parsed: attrs.parsed,
        pinned,
        follow,
    })
}

/// Whether `node` (a fetcher call) is the `src =` attribute inside a
/// derivation-wrapper function (`mkDerivation`, `buildRustPackage`, ...),
/// in which case the corresponding derivation rule owns it, not the
/// fetcher rule.
pub(super) fn is_src_of_owned_call(node: &NixNode) -> bool {
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

    let mut apply_node = match attr_set.parent() {
        Some(p) => p,
        None => return false,
    };
    if apply_node.kind() == rnix::SyntaxKind::NODE_LAMBDA {
        apply_node = match apply_node.parent() {
            Some(p) => p,
            None => return false,
        };
    }
    if apply_node.kind() == rnix::SyntaxKind::NODE_PAREN {
        apply_node = match apply_node.parent() {
            Some(p) => p,
            None => return false,
        };
    }
    if apply_node.kind() != rnix::SyntaxKind::NODE_APPLY {
        return false;
    }

    let func_name = match apply_node.apply_function_name() {
        Some(name) => name,
        None => return false,
    };
    let short_name = func_name.rsplit('.').next().unwrap_or(&func_name);
    OWNED_FUNC_NAMES.contains(&short_name)
}

#[cfg(test)]
mod tests {
    use crate::parser::NixFile;

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
        assert!(super::is_src_of_owned_call(&fetcher_node));
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
        assert!(!super::is_src_of_owned_call(&fetcher_node));
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
        assert!(super::is_src_of_owned_call(&fetcher_node));
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
        assert!(!super::is_src_of_owned_call(&fetcher_node));
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
        assert!(!super::is_src_of_owned_call(&fetcher_node));
    }

    #[test]
    fn test_is_src_of_build_rust_package_returns_true() {
        let content = r#"
rustPlatform.buildRustPackage rec {
  pname = "foo";
  version = "1.0.0";
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "0000000000000000000000000000000000000000";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
  cargoHash = "";
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::is_src_of_owned_call(&fetcher_node));
    }

    #[test]
    fn test_is_src_of_lambda_wrapped_mk_derivation_returns_true() {
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
        assert!(super::is_src_of_owned_call(&fetcher_node));
    }

    #[test]
    fn test_fetcher_skips_interpolated_operational_key() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/${name}";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        assert!(super::try_extract_call(&fetcher_node).is_none());
    }

    #[test]
    fn test_fetcher_allows_unknown_interpolated_key() {
        let content = r#"
{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v1.0.0";
    hash = "sha256-AAA=";
    name = "foo-${version}";
  };
}
"#;
        let root = parse_root(content);
        let fetcher_node = find_fetcher_apply(&root, "fetchgit").unwrap();
        let call = super::try_extract_call(&fetcher_node);
        assert!(call.is_some());
        assert_eq!(
            call.unwrap().parsed.strings.get("rev"),
            Some(&"v1.0.0".to_string())
        );
    }
}
