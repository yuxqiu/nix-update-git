//! AST extraction: turning `rec { version = "..."; src = fetchX { ... }; }`
//! (or a lambda-wrapped equivalent) into a `DerivationCall`.

use std::collections::HashMap;

use crate::parser::{NixNode, ParsedAttrs, TextRange};
use crate::rules::fetcher::{InterpolationSpec, kind::FetcherKind, parse_fetcher_attrset};
use crate::utils::VersionDetector;

pub(super) struct DerivationCall {
    pub(super) version_value: String,
    pub(super) version_range: TextRange,
    pub(super) source_ref_key: Option<String>,
    pub(super) source_ref_value: SourceRefValue,
    pub(super) source_ref_range: Option<TextRange>,
    pub(super) fetcher_kind: FetcherKind,
    pub(super) fetcher_parsed: ParsedAttrs,
    pub(super) extra_vars: HashMap<String, String>,
    pub(super) pinned: bool,
}

pub(super) enum SourceRefValue {
    Missing,
    Pure(String),
    IdentFromVersion,
    InterpolatedFromVersion {
        template_node: NixNode,
        version_var: String,
    },
}

pub(super) fn try_extract_call(func_names: &[String], node: &NixNode) -> Option<DerivationCall> {
    let func_name = node.apply_function_name()?;
    let short_name = func_name.rsplit('.').next().unwrap_or(&func_name);
    if !func_names.iter().any(|n| n == short_name) {
        return None;
    }

    let arg = node.apply_argument_attrset()?;

    let version_entry = arg.find_attr_by_key("version")?;
    let version_node = version_entry.attr_value()?;
    if version_node.kind() != rnix::SyntaxKind::NODE_STRING {
        return None;
    }
    let version_content = version_node.pure_string_content()?;
    if !VersionDetector::is_version(&version_content) {
        return None;
    }

    let mut stable_attrs: HashMap<String, String> = HashMap::new();
    for child in arg.children() {
        if child.kind() != rnix::SyntaxKind::NODE_ATTRPATH_VALUE {
            continue;
        }
        let segments = child.attrpath_segments();
        if segments.len() != 1 {
            continue;
        }
        let key = segments[0].clone();
        if key == "version" {
            continue;
        }
        if let Some(value) = child.attr_value()
            && let Some(content) = value.pure_string_content()
        {
            stable_attrs.insert(key, content);
        }
    }

    let is_recursive = arg.text().trim_start().starts_with("rec");

    let lambda_param = node.apply_lambda_param();

    let version_vars: Vec<String> = {
        let mut vars = Vec::new();
        if is_recursive {
            vars.push("version".to_string());
        }
        if let Some(ref param) = lambda_param {
            vars.push(format!("{}.version", param));
        }
        vars
    };

    let src_entry = arg.find_attr_by_key("src")?;
    let src_value = src_entry.attr_value()?;
    if src_value.kind() != rnix::SyntaxKind::NODE_APPLY {
        return None;
    }

    let src_func_name = src_value.apply_function_name()?;
    let fetcher_kind = FetcherKind::from_name(&src_func_name)?;

    let src_arg = src_value.apply_argument()?;
    if src_arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
        return None;
    }

    let mut ident_vars: HashMap<String, String> = HashMap::new();
    let mut interpolation_vars: HashMap<String, String> = HashMap::new();
    if is_recursive {
        for (key, value) in &stable_attrs {
            ident_vars.insert(key.clone(), value.clone());
            interpolation_vars.insert(key.clone(), value.clone());
        }
        ident_vars.insert("version".to_string(), version_content.clone());
    }
    if let Some(ref param) = lambda_param {
        for (key, value) in &stable_attrs {
            if is_recursive {
                ident_vars.insert(key.clone(), value.clone());
            }
            let dotted = format!("{}.{}", param, key);
            interpolation_vars.insert(dotted, value.clone());
        }
        if is_recursive {
            ident_vars.insert("version".to_string(), version_content.clone());
        }
        let dotted_version = format!("{}.version", param);
        ident_vars.insert(dotted_version, version_content.clone());
    }

    let mut spec = InterpolationSpec::none();
    if !interpolation_vars.is_empty() {
        spec.allow_all(interpolation_vars.clone());
    }
    if !ident_vars.is_empty() {
        spec.allow_idents(ident_vars);
    }
    if !version_vars.is_empty() {
        let vars: HashMap<String, String> = version_vars
            .iter()
            .map(|v| (v.clone(), version_content.clone()))
            .collect();
        spec.allow("tag", vars.clone());
        spec.allow("rev", vars.clone());
        spec.allow("ref", vars);
    }

    let mut attrs = match parse_fetcher_attrset(fetcher_kind, &src_arg, &spec) {
        Ok(a) => a,
        Err(_) => return None,
    };

    let source_ref_keys = ["tag", "rev", "ref"];
    let resolved_keys: Vec<String> = attrs
        .interpolated
        .keys()
        .filter(|k| !source_ref_keys.contains(&k.as_str()))
        .cloned()
        .collect();
    for key in resolved_keys {
        if let Some(template) = attrs.interpolated.remove(&key) {
            if let Some(resolved) = template.interpolated_string_content(&interpolation_vars) {
                attrs.parsed.strings.insert(key, resolved);
            } else {
                attrs.interpolated.insert(key, template);
            }
        }
    }

    let op_keys = fetcher_kind.operational_keys();
    if attrs
        .interpolated_unresolved
        .iter()
        .any(|k| op_keys.contains(&k.as_str()))
    {
        return None;
    }

    let source_ref_key = crate::rules::fetcher::preferred_ref_key(&attrs.parsed)
        .map(|k| k.to_string())
        .or_else(|| {
            if attrs.interpolated.contains_key("tag") {
                Some("tag".to_string())
            } else if attrs.interpolated.contains_key("rev") {
                Some("rev".to_string())
            } else if attrs.interpolated.contains_key("ref") {
                Some("ref".to_string())
            } else {
                None
            }
        });

    let source_ref_value = if let Some(key) = &source_ref_key {
        if let Some(ident_name) = attrs.parsed.ident_resolved.get(key)
            && version_vars.iter().any(|v| v == ident_name)
        {
            SourceRefValue::IdentFromVersion
        } else if let Some(value) = attrs.parsed.strings.get(key) {
            SourceRefValue::Pure(value.clone())
        } else if let Some(template_node) = attrs.interpolated.remove(key) {
            let detected_var = version_vars
                .iter()
                .find(|v| {
                    template_node
                        .interpolated_var_affixes(v, &interpolation_vars)
                        .is_some()
                })
                .cloned()
                .unwrap_or_else(|| version_vars.first().cloned().unwrap_or_default());
            SourceRefValue::InterpolatedFromVersion {
                template_node,
                version_var: detected_var,
            }
        } else {
            SourceRefValue::Missing
        }
    } else {
        SourceRefValue::Missing
    };

    let source_ref_range = source_ref_key
        .as_ref()
        .and_then(|key| attrs.parsed.string_range(key));

    let pinned = arg.has_pin_comment()
        || node.has_pin_comment()
        || src_arg.has_pin_comment()
        || src_value.has_pin_comment();

    Some(DerivationCall {
        version_value: version_content,
        version_range: version_node.text_range(),
        source_ref_key,
        source_ref_value,
        source_ref_range,
        fetcher_kind,
        fetcher_parsed: attrs.parsed,
        extra_vars: interpolation_vars,
        pinned,
    })
}
