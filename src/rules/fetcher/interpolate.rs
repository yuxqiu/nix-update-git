//! Variable-interpolation resolution for fetcher attribute sets.

use std::collections::HashMap;

use crate::parser::{NixNode, ParsedAttrs};

use super::kind::FetcherKind;

/// Controls which fetcher fields may contain interpolation: `allow()` for
/// field-specific bindings, `allow_all()` for catch-all bindings merged on
/// top, `allow_idents()` for bare ident resolution (e.g. `repo = pname`).
pub struct InterpolationSpec {
    allowed: HashMap<String, HashMap<String, String>>,
    allow_all_vars: Option<HashMap<String, String>>,
    ident_vars: HashMap<String, String>,
}

impl InterpolationSpec {
    pub(crate) fn none() -> Self {
        Self {
            allowed: HashMap::new(),
            allow_all_vars: None,
            ident_vars: HashMap::new(),
        }
    }

    pub(crate) fn allow(&mut self, field: &str, vars: HashMap<String, String>) {
        self.allowed.insert(field.to_string(), vars);
    }

    pub(crate) fn allow_all(&mut self, vars: HashMap<String, String>) {
        self.allow_all_vars = Some(vars);
    }

    pub(crate) fn allow_idents(&mut self, idents: HashMap<String, String>) {
        self.ident_vars = idents;
    }

    pub(crate) fn vars_for_field(&self, field: &str) -> Option<HashMap<String, String>> {
        match (&self.allow_all_vars, self.allowed.get(field)) {
            (None, None) => None,
            (None, Some(field_vars)) => Some(field_vars.clone()),
            (Some(default_vars), None) => Some(default_vars.clone()),
            (Some(default_vars), Some(field_vars)) => {
                let mut merged = default_vars.clone();
                merged.extend(field_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
                Some(merged)
            }
        }
    }
}

pub struct FetcherAttrs {
    pub parsed: ParsedAttrs,
    pub interpolated: HashMap<String, NixNode>,
    pub interpolated_unresolved: Vec<String>,
}

pub fn parse_fetcher_attrset(
    kind: FetcherKind,
    attr_set: &NixNode,
    spec: &InterpolationSpec,
) -> anyhow::Result<FetcherAttrs> {
    let ident_vars_opt = if spec.ident_vars.is_empty() {
        None
    } else {
        Some(&spec.ident_vars)
    };
    let parsed = attr_set.parse_attrs(kind.attr_spec(), ident_vars_opt)?;

    let mut interpolated = HashMap::new();
    let mut interpolated_unresolved = Vec::new();

    for (key, node) in &parsed.string_nodes {
        if node.pure_string_content().is_some() {
            // Already handled by parse_attrs -> strings
        } else if let Some(vars) = spec.vars_for_field(key) {
            if node.interpolated_string_content(&vars).is_some() {
                interpolated.insert(key.clone(), node.clone());
            } else {
                interpolated_unresolved.push(key.clone());
            }
        } else {
            interpolated_unresolved.push(key.clone());
        }
    }

    Ok(FetcherAttrs {
        parsed,
        interpolated,
        interpolated_unresolved,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::parser::NixFile;

    fn parse_root(content: &str) -> crate::parser::NixNode {
        NixFile::parse(content).unwrap().root_node()
    }

    #[test]
    fn test_parse_fetcher_attrset_pure_strings() {
        let content = r#"{ url = "https://example.com"; rev = "v1.0"; fetchSubmodules = true; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::FetchGit,
            &attr_set,
            &super::InterpolationSpec::none(),
        )
        .unwrap();
        assert_eq!(
            attrs.parsed.strings.get("url"),
            Some(&"https://example.com".to_string())
        );
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0".to_string()));
        assert_eq!(attrs.parsed.bools.get("fetchSubmodules"), Some(&true));
        assert!(attrs.interpolated.is_empty());
        assert_eq!(
            attrs.interpolated_unresolved,
            [] as [std::string::String; 0]
        );
        assert!(attrs.parsed.has_string("url"));
        assert!(attrs.parsed.has_string("rev"));
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_unresolved_with_no_spec() {
        let content = r#"{ url = "https://example.com/${name}"; rev = "v1.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::FetchGit,
            &attr_set,
            &super::InterpolationSpec::none(),
        )
        .unwrap();
        assert!(!attrs.parsed.strings.contains_key("url"));
        assert_eq!(attrs.interpolated_unresolved, vec!["url"]);
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0".to_string()));
        assert!(attrs.interpolated.is_empty());
        assert!(attrs.parsed.has_string("url"));
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_allowed_by_spec() {
        let content = r#"{ rev = "v${version}"; version = "1.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert_eq!(
            attrs.interpolated_unresolved,
            [] as [std::string::String; 0]
        );
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(
            attrs.parsed.strings.get("version"),
            Some(&"1.0".to_string())
        );
    }

    #[test]
    fn test_parse_fetcher_attrset_interpolated_not_matching_spec() {
        let content = r#"{ rev = "v${unknown}"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.is_empty());
        assert_eq!(attrs.interpolated_unresolved, vec!["rev"]);
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars() {
        let content = r#"{ rev = "v${version}"; owner = "test"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([
                ("version".to_string(), "1.0".to_string()),
                ("finalAttrs.version".to_string(), "1.0".to_string()),
            ]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert_eq!(
            attrs.interpolated_unresolved,
            [] as [std::string::String; 0]
        );
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(attrs.parsed.strings.get("owner"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_dual_interpolation_vars_dotted() {
        let content = r#"{ rev = "v${finalAttrs.version}"; owner = "test"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([
                ("version".to_string(), "1.0".to_string()),
                ("finalAttrs.version".to_string(), "1.0".to_string()),
            ]),
        );
        let attrs =
            super::parse_fetcher_attrset(super::FetcherKind::FetchGit, &attr_set, &spec).unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert_eq!(
            attrs.interpolated_unresolved,
            [] as [std::string::String; 0]
        );
        assert!(!attrs.parsed.strings.contains_key("rev"));
        assert_eq!(attrs.parsed.strings.get("owner"), Some(&"test".to_string()));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_merge() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let rev_vars = spec.vars_for_field("rev").unwrap();
        assert_eq!(rev_vars.get("pname"), Some(&"foo".to_string()));
        assert_eq!(rev_vars.get("version"), Some(&"1.0".to_string()));
        let owner_vars = spec.vars_for_field("owner").unwrap();
        assert_eq!(owner_vars.get("pname"), Some(&"foo".to_string()));
        assert!(!owner_vars.contains_key("version"));
        let unknown_vars = spec.vars_for_field("name").unwrap();
        assert_eq!(unknown_vars.get("pname"), Some(&"foo".to_string()));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_none() {
        let spec = super::InterpolationSpec::none();
        assert!(spec.vars_for_field("rev").is_none());
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_only_allow_all() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        let vars = spec.vars_for_field("owner").unwrap();
        assert_eq!(vars.get("pname"), Some(&"foo".to_string()));
    }

    #[test]
    fn test_interpolation_spec_vars_for_field_only_field_specific() {
        let mut spec = super::InterpolationSpec::none();
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let rev_vars = spec.vars_for_field("rev").unwrap();
        assert_eq!(rev_vars.get("version"), Some(&"1.0".to_string()));
        assert!(spec.vars_for_field("owner").is_none());
    }

    #[test]
    fn test_parse_fetcher_attrset_ident_resolution() {
        let content = r#"{ repo = pname; owner = "test-org"; rev = "v1.0.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_idents(HashMap::from([("pname".to_string(), "my-pkg".to_string())]));
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::from_name("fetchFromGitHub").unwrap(),
            &attr_set,
            &spec,
        )
        .unwrap();
        assert_eq!(
            attrs.parsed.strings.get("repo"),
            Some(&"my-pkg".to_string())
        );
        assert_eq!(
            attrs.parsed.strings.get("owner"),
            Some(&"test-org".to_string())
        );
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_ident_not_in_idents_returns_error() {
        let content = r#"{ repo = pname; owner = "test-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let spec = super::InterpolationSpec::none();
        let result = super::parse_fetcher_attrset(
            super::FetcherKind::from_name("fetchFromGitHub").unwrap(),
            &attr_set,
            &spec,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fetcher_attrset_select_resolution() {
        let content = r#"{ rev = finalAttrs.version; owner = "test-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_idents(HashMap::from([(
            "finalAttrs.version".to_string(),
            "1.0.0".to_string(),
        )]));
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::from_name("fetchFromGitHub").unwrap(),
            &attr_set,
            &spec,
        )
        .unwrap();
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"1.0.0".to_string()));
        assert_eq!(
            attrs.parsed.strings.get("owner"),
            Some(&"test-org".to_string())
        );
        assert_eq!(
            attrs.parsed.ident_resolved.get("rev"),
            Some(&"finalAttrs.version".to_string())
        );
    }

    #[test]
    fn test_parse_fetcher_attrset_allow_all_interpolation() {
        let content = r#"{ owner = "${pname}-org"; rev = "v1.0.0"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::from_name("fetchFromGitHub").unwrap(),
            &attr_set,
            &spec,
        )
        .unwrap();
        assert!(attrs.interpolated.contains_key("owner"));
        assert!(!attrs.interpolated_unresolved.iter().any(|k| k == "owner"));
        assert_eq!(attrs.parsed.strings.get("rev"), Some(&"v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_fetcher_attrset_allow_all_and_field_specific_merge() {
        let content = r#"{ rev = "${pname}-${version}"; owner = "${pname}-org"; }"#;
        let root = parse_root(content);
        let attr_set = root
            .traverse()
            .find(|n| n.kind() == rnix::SyntaxKind::NODE_ATTR_SET)
            .unwrap();
        let mut spec = super::InterpolationSpec::none();
        spec.allow_all(HashMap::from([("pname".to_string(), "foo".to_string())]));
        spec.allow(
            "rev",
            HashMap::from([("version".to_string(), "1.0".to_string())]),
        );
        let attrs = super::parse_fetcher_attrset(
            super::FetcherKind::from_name("fetchFromGitHub").unwrap(),
            &attr_set,
            &spec,
        )
        .unwrap();
        assert!(attrs.interpolated.contains_key("rev"));
        assert!(attrs.interpolated.contains_key("owner"));
        assert_eq!(
            attrs.interpolated_unresolved,
            [] as [std::string::String; 0]
        );
    }
}
