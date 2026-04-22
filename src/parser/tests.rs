#![cfg(test)]

use crate::parser::{AttrSpec, AttrType, NixFile, NixNode};
use std::collections::HashMap;

fn parse(content: &str) -> NixNode {
    NixFile::parse(content).unwrap().root_node()
}

fn find_attr_set(root: &NixNode) -> Option<NixNode> {
    root.children()
        .into_iter()
        .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
}

#[test]
fn test_pure_string_content_double_quoted() {
    let content = r#"{ foo = "hello world"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.pure_string_content(), Some("hello world".to_string()));
}

#[test]
fn test_pure_string_content_indented() {
    let content = "foo = ''\n  hello\n  world\n'';\n";
    let full_content = format!("{{ {} }}", content);
    let root = parse(&full_content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(
        value.pure_string_content(),
        Some("hello\nworld\n".to_string())
    );
}

#[test]
fn test_pure_string_content_empty() {
    let content = r#"{ foo = ""; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.pure_string_content(), Some("".to_string()));
}

#[test]
fn test_pure_string_content_escape_sequences() {
    let content = r#"{ foo = "line1\nline2\ttab"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(
        value.pure_string_content(),
        Some("line1\nline2\ttab".to_string())
    );
}

#[test]
fn test_pure_string_content_non_string() {
    let content = "{ foo = 123; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.pure_string_content(), None);
}

#[test]
fn test_pure_string_content_interpolated_returns_none() {
    let content = r#"{ foo = "hello ${name}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.pure_string_content(), None);
}

#[test]
fn test_interpolated_string_content_with_vars() {
    let content = r#"{ name = "world"; foo = "hello ${name}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    let mut vars = std::collections::HashMap::new();
    vars.insert("name".to_string(), "world".to_string());
    assert_eq!(
        value.interpolated_string_content(&vars),
        Some("hello world".to_string())
    );
}

#[test]
fn test_interpolated_string_content_missing_var_returns_none() {
    let content = r#"{ foo = "hello ${name}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    assert_eq!(value.interpolated_string_content(&vars), None);
}

#[test]
fn test_interpolated_string_content_non_string_returns_none() {
    let content = "{ foo = 123; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    assert_eq!(value.interpolated_string_content(&vars), None);
}

#[test]
fn test_attrpath_segments_simple() {
    let content = "{ foo = 1; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for child in attr_set.children() {
        if child.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            let segments = child.attrpath_segments();
            assert_eq!(segments, vec!["foo"]);
        }
    }
}

#[test]
fn test_attrpath_segments_dotted() {
    let content = "{ inputs.mylib.ref = \"v1.0\"; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            let segments = node.attrpath_segments();
            if !segments.is_empty() && segments[0] == "inputs" {
                assert_eq!(segments, vec!["inputs", "mylib", "ref"]);
            }
        }
    }
}

#[test]
fn test_has_pin_comment_on_node() {
    let content = r#"
{
  foo = {
    url = "github:owner/repo";
    ref = "v1.0.0"; # pin
  };
}
"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            let segments = node.attrpath_segments();
            if segments == vec!["foo", "ref"] {
                assert!(node.has_pin_comment());
            }
        }
    }
}

#[test]
fn test_has_pin_comment_absent() {
    let content = r#"
{
  foo = {
    url = "github:owner/repo";
    ref = "v1.0.0";
  };
}
"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            let segments = node.attrpath_segments();
            if segments == vec!["foo", "ref"] {
                assert!(!node.has_pin_comment());
            }
        }
    }
}

#[test]
fn test_follow_branch_comment_absent() {
    let content = "{ src = fetchgit { url = \"https://example.com/repo\"; rev = \"abc123\"; }; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_APPLY {
            let name = node.apply_function_name();
            if name.as_deref() == Some("fetchgit") {
                assert_eq!(node.follow_branch_comment(), None);
            }
        }
    }
}

#[test]
fn test_follow_branch_comment_present() {
    let content = "{ src = fetchgit { # follow:main\n    url = \"https://example.com/repo\"; rev = \"abc123\"; }; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_ATTR_SET
            && let Some(branch) = node.follow_branch_comment()
        {
            assert_eq!(branch, "main".to_string());
            return;
        }
    }
    panic!("Expected to find follow:main comment on attr set node");
}

#[test]
fn test_apply_function_name_simple() {
    let content = "{ src = fetchgit { url = \"...\"; }; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_APPLY {
            assert_eq!(node.apply_function_name(), Some("fetchgit".to_string()));
        }
    }
}

#[test]
fn test_apply_function_name_dotted() {
    let content = "{ src = pkgs.fetchFromGitHub { owner = \"foo\"; repo = \"bar\"; }; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    for node in attr_set.traverse() {
        if node.kind() == rnix::SyntaxKind::NODE_APPLY {
            assert_eq!(
                node.apply_function_name(),
                Some("pkgs.fetchFromGitHub".to_string())
            );
        }
    }
}

#[test]
fn test_apply_function_name_nested() {
    let content = "{ src = lib.getExe (pkgs.fetchgit { url = \"...\"; }); }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let found = attr_set
        .traverse()
        .find(|n| n.kind() == rnix::SyntaxKind::NODE_APPLY);
    assert!(found.is_some());
}

#[test]
fn test_apply_argument_attrset_direct() {
    let content = "{ x = f { foo = \"bar\"; }; }";
    let root = parse(content);
    let apply = root
        .traverse()
        .find(|n| n.kind() == rnix::SyntaxKind::NODE_APPLY)
        .unwrap();
    let arg = apply.apply_argument_attrset().unwrap();
    assert_eq!(arg.kind(), rnix::SyntaxKind::NODE_ATTR_SET);
    assert_eq!(arg.find_string_value("foo"), Some("bar".to_string()));
}

#[test]
fn test_apply_argument_attrset_lambda_wrapped() {
    let content = "{ x = stdenv.mkDerivation (finalAttrs: { version = \"1.0\"; }); }";
    let root = parse(content);
    let apply = root
        .traverse()
        .find(|n| {
            n.kind() == rnix::SyntaxKind::NODE_APPLY
                && n.apply_function_name().as_deref() == Some("stdenv.mkDerivation")
        })
        .unwrap();
    // apply_argument returns None because there's no direct ATTR_SET child
    assert!(apply.apply_argument().is_none());
    // apply_argument_attrset unwraps the paren/lambda to find the attrset
    let arg = apply.apply_argument_attrset().unwrap();
    assert_eq!(arg.kind(), rnix::SyntaxKind::NODE_ATTR_SET);
    assert_eq!(arg.find_string_value("version"), Some("1.0".to_string()));
}

#[test]
fn test_apply_lambda_param_present() {
    let content = "{ x = stdenv.mkDerivation (finalAttrs: { version = \"1.0\"; }); }";
    let root = parse(content);
    let apply = root
        .traverse()
        .find(|n| {
            n.kind() == rnix::SyntaxKind::NODE_APPLY
                && n.apply_function_name().as_deref() == Some("stdenv.mkDerivation")
        })
        .unwrap();
    assert_eq!(apply.apply_lambda_param(), Some("finalAttrs".to_string()));
}

#[test]
fn test_apply_lambda_param_absent_direct_attrset() {
    let content = "{ x = stdenv.mkDerivation rec { version = \"1.0\"; }; }";
    let root = parse(content);
    let apply = root
        .traverse()
        .find(|n| {
            n.kind() == rnix::SyntaxKind::NODE_APPLY
                && n.apply_function_name().as_deref() == Some("stdenv.mkDerivation")
        })
        .unwrap();
    // Direct attrset — no lambda parameter
    assert!(apply.apply_lambda_param().is_none());
}

#[test]
fn test_find_string_value() {
    let content = r#"
{
  foo = "bar";
  nested = {
    key = "value";
  };
}
"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    assert_eq!(attr_set.find_string_value("foo"), Some("bar".to_string()));
}

#[test]
fn test_find_string_value_not_found() {
    let content = "{ foo = \"bar\"; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    assert_eq!(attr_set.find_string_value("nonexistent"), None);
}

#[test]
fn test_find_bool_value_true() {
    let content = "{ fetchSubmodules = true; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    assert_eq!(attr_set.find_bool_value("fetchSubmodules"), Some(true));
}

#[test]
fn test_find_bool_value_false() {
    let content = "{ fetchSubmodules = false; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    assert_eq!(attr_set.find_bool_value("fetchSubmodules"), Some(false));
}

#[test]
fn test_find_bool_value_non_bool() {
    let content = "{ foo = 123; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    assert_eq!(attr_set.find_bool_value("foo"), None);
}

#[test]
fn test_interpolated_var_affixes_single_var() {
    let content = r#"{ rev = "v${version}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    assert_eq!(
        value.interpolated_var_affixes("version", &vars),
        Some(("v".to_string(), "".to_string()))
    );
}

#[test]
fn test_interpolated_var_affixes_multi_var() {
    let content = r#"{ rev = "${pname}-${version}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let mut vars = std::collections::HashMap::new();
    vars.insert("pname".to_string(), "foo".to_string());
    assert_eq!(
        value.interpolated_var_affixes("version", &vars),
        Some(("foo-".to_string(), "".to_string()))
    );
}

#[test]
fn test_interpolated_var_affixes_multi_var_with_suffix() {
    let content = r#"{ rev = "${pname}-${version}-release"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let mut vars = std::collections::HashMap::new();
    vars.insert("pname".to_string(), "foo".to_string());
    assert_eq!(
        value.interpolated_var_affixes("version", &vars),
        Some(("foo-".to_string(), "-release".to_string()))
    );
}

#[test]
fn test_interpolated_var_affixes_missing_stable_var() {
    let content = r#"{ rev = "${pname}-${version}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    // pname not in vars → cannot resolve → None
    assert_eq!(value.interpolated_var_affixes("version", &vars), None);
}

#[test]
fn test_interpolated_var_affixes_target_var_twice() {
    let content = r#"{ rev = "${version}-${version}"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    // version appears twice → None
    assert_eq!(value.interpolated_var_affixes("version", &vars), None);
}

#[test]
fn test_interpolated_var_affixes_no_target_var() {
    let content = r#"{ rev = "${pname}-src"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("rev").unwrap();
    let value = node.attr_value().unwrap();
    let mut vars = std::collections::HashMap::new();
    vars.insert("pname".to_string(), "foo".to_string());
    // version not present → None
    assert_eq!(value.interpolated_var_affixes("version", &vars), None);
}

#[test]
fn test_interpolated_var_affixes_non_string() {
    let content = "{ foo = 123; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    let vars = std::collections::HashMap::new();
    assert_eq!(value.interpolated_var_affixes("version", &vars), None);
}

#[test]
fn test_parse_attrs_pure_strings() {
    let content = r#"{ url = "https://example.com"; rev = "v1.0"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "url",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "rev",
            attr_type: AttrType::String,
        },
    ];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(
        parsed.strings.get("url"),
        Some(&"https://example.com".to_string())
    );
    assert_eq!(parsed.strings.get("rev"), Some(&"v1.0".to_string()));
    assert!(parsed.bools.is_empty());
    assert!(parsed.ints.is_empty());
    assert!(parsed.has_string("url"));
    assert!(parsed.has_string("rev"));
    assert!(parsed.unknown_keys.is_empty());
}

#[test]
fn test_parse_attrs_bool_values() {
    let content = r#"{ fetchSubmodules = true; deepClone = false; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "fetchSubmodules",
            attr_type: AttrType::Bool,
        },
        AttrSpec {
            key: "deepClone",
            attr_type: AttrType::Bool,
        },
    ];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(parsed.bools.get("fetchSubmodules"), Some(&true));
    assert_eq!(parsed.bools.get("deepClone"), Some(&false));
    assert!(parsed.strings.is_empty());
    assert!(parsed.unknown_keys.is_empty());
}

#[test]
fn test_parse_attrs_int_values() {
    let content = "{ stripLen = 1; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[AttrSpec {
        key: "stripLen",
        attr_type: AttrType::Int,
    }];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(parsed.ints.get("stripLen"), Some(&1i64));
    assert!(parsed.ints.contains_key("stripLen"));
}

#[test]
fn test_parse_attrs_list_strings() {
    let content = r#"{ sparseCheckout = [ "path/to/dir" "another/dir" ]; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[AttrSpec {
        key: "sparseCheckout",
        attr_type: AttrType::ListString,
    }];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(
        parsed.pure_string_list("sparseCheckout"),
        Some(vec!["path/to/dir".to_string(), "another/dir".to_string()])
    );
}

#[test]
fn test_parse_attrs_list_ints() {
    let content = "{ hunks = [ 1 2 3 ]; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[AttrSpec {
        key: "hunks",
        attr_type: AttrType::ListInt,
    }];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(parsed.list_ints.get("hunks"), Some(&vec![1i64, 2i64, 3i64]));
}

#[test]
fn test_parse_attrs_unknown_keys_parsed_as_strings() {
    let content = r#"{ url = "https://example.com"; unknown_key = "hello"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    }];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(
        parsed.strings.get("url"),
        Some(&"https://example.com".to_string())
    );
    assert_eq!(
        parsed.strings.get("unknown_key"),
        Some(&"hello".to_string())
    );
    assert!(parsed.unknown_keys.contains(&"unknown_key".to_string()));
}

#[test]
fn test_parse_attrs_type_mismatch_returns_error() {
    let content = "{ url = 123; fetchSubmodules = true; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "url",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "rev",
            attr_type: AttrType::String,
        },
    ];
    let result = attr_set.parse_attrs(spec, None);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("url"));
    assert!(err.to_string().contains("string"));
}

#[test]
fn test_parse_attrs_ident_resolution() {
    let content = r#"{ repo = pname; owner = "test-org"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "repo",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "owner",
            attr_type: AttrType::String,
        },
    ];
    let ident_vars = HashMap::from([("pname".to_string(), "my-pkg".to_string())]);
    let parsed = attr_set.parse_attrs(spec, Some(&ident_vars)).unwrap();
    assert_eq!(parsed.strings.get("repo"), Some(&"my-pkg".to_string()));
    assert_eq!(parsed.strings.get("owner"), Some(&"test-org".to_string()));
}

#[test]
fn test_parse_attrs_ident_not_in_vars_returns_error() {
    let content = r#"{ repo = pname; owner = "test-org"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "repo",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "owner",
            attr_type: AttrType::String,
        },
    ];
    let result = attr_set.parse_attrs(spec, None::<&HashMap<String, String>>);
    assert!(result.is_err());
}

#[test]
fn test_parse_attrs_interpolated_string_in_string_nodes() {
    let content = r#"{ url = "https://example.com/${name}"; rev = "v1.0"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "url",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "rev",
            attr_type: AttrType::String,
        },
    ];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert!(!parsed.strings.contains_key("url"));
    assert!(parsed.string_nodes.contains_key("url"));
    assert_eq!(parsed.strings.get("rev"), Some(&"v1.0".to_string()));
}

#[test]
fn test_parse_attrs_mixed_spec() {
    let content = r#"{ url = "https://example.com"; rev = "v1.0"; fetchSubmodules = true; stripLen = 2; sparseCheckout = [ "dir" ]; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[
        AttrSpec {
            key: "url",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "rev",
            attr_type: AttrType::String,
        },
        AttrSpec {
            key: "fetchSubmodules",
            attr_type: AttrType::Bool,
        },
        AttrSpec {
            key: "stripLen",
            attr_type: AttrType::Int,
        },
        AttrSpec {
            key: "sparseCheckout",
            attr_type: AttrType::ListString,
        },
    ];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert_eq!(
        parsed.strings.get("url"),
        Some(&"https://example.com".to_string())
    );
    assert_eq!(parsed.strings.get("rev"), Some(&"v1.0".to_string()));
    assert_eq!(parsed.bools.get("fetchSubmodules"), Some(&true));
    assert_eq!(parsed.ints.get("stripLen"), Some(&2i64));
    assert_eq!(
        parsed.pure_string_list("sparseCheckout"),
        Some(vec!["dir".to_string()])
    );
}

#[test]
fn test_parse_attrs_empty_attrset() {
    let content = "{}";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let spec = &[AttrSpec {
        key: "url",
        attr_type: AttrType::String,
    }];
    let parsed = attr_set.parse_attrs(spec, None).unwrap();
    assert!(parsed.strings.is_empty());
    assert!(parsed.bools.is_empty());
    assert!(parsed.ints.is_empty());
    assert!(parsed.unknown_keys.is_empty());
}
