#![cfg(test)]

use crate::parser::{NixFile, NixNode};

fn parse(content: &str) -> NixNode {
    NixFile::parse(content).unwrap().root_node()
}

fn find_attr_set(root: &NixNode) -> Option<NixNode> {
    root.children()
        .into_iter()
        .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
}

#[test]
fn test_string_content_double_quoted() {
    let content = "{ foo = \"hello world\"; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.string_content(), Some("hello world".to_string()));
}

#[test]
fn test_string_content_indented() {
    let content = "foo = ''\n  hello\n  world\n'';\n";
    let full_content = format!("{{ {} }}", content);
    let root = parse(&full_content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(
        value.string_content(),
        Some("\n  hello\n  world\n".to_string())
    );
}

#[test]
fn test_string_content_empty() {
    let content = r#"{ foo = ""; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.string_content(), Some("".to_string()));
}

#[test]
fn test_string_content_escape_sequences() {
    let content = r#"{ foo = "line1\nline2\ttab"; }"#;
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(
        value.string_content(),
        Some("line1\\nline2\\ttab".to_string())
    );
}

#[test]
fn test_string_content_non_string() {
    let content = "{ foo = 123; }";
    let root = parse(content);
    let attr_set = find_attr_set(&root).unwrap();
    let node = attr_set.find_attr_by_key("foo").unwrap();
    let value = node.attr_value().unwrap();
    assert_eq!(value.string_content(), None);
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
