use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, TextRange};
use crate::rules::fetcher::{
    git_fetch, is_commit_hash, kind::FetcherKind, kind::HashStrategy, preferred_ref_key,
    resolve_ref_for_prefetch, tarball,
};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, VersionDetector};

// Resolution strategy:
// 1) Select source ref key with fetcher precedence: tag > rev > ref.
// 2) Derive update intent primarily from that source ref (pure or interpolated);
//    fall back to mkDerivation.version for empty/hash refs.
// 3) When source ref changes (or hash is empty), recompute hash/sha256 using the
//    resolved fetch revision. For interpolated refs, only version is rewritten.

struct MkDerivationCall {
    version_value: String,
    version_range: TextRange,
    source_ref_key: Option<String>,
    source_ref_value: SourceRefValue,
    source_ref_range: Option<TextRange>,
    fetcher_kind: FetcherKind,
    fetcher_params: HashMap<String, String>,
    fetcher_source_ranges: HashMap<String, TextRange>,
    fetcher_sparse_checkout: Vec<String>,
    pinned: bool,
}

enum SourceRefValue {
    Missing,
    Pure(String),
    InterpolatedFromVersion { template_node: NixNode },
}

pub struct MkDerivationRule;

impl MkDerivationRule {
    pub fn new() -> Self {
        Self
    }

    fn try_extract_call(node: &NixNode) -> Option<MkDerivationCall> {
        let func_name = node.apply_function_name()?;
        let short_name = func_name.rsplit('.').next().unwrap_or(&func_name);
        if short_name != "mkDerivation" {
            return None;
        }

        let arg = node.apply_argument()?;
        if arg.kind() != rnix::SyntaxKind::NODE_ATTR_SET {
            return None;
        }

        let version_entry = arg.find_attr_by_key("version")?;
        let version_node = version_entry.attr_value()?;
        if version_node.kind() != rnix::SyntaxKind::NODE_STRING {
            return None;
        }
        let version_content = version_node.pure_string_content()?;
        if !VersionDetector::is_version(&version_content) {
            return None;
        }
        let is_recursive = arg.text().trim_start().starts_with("rec");

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

        let mut params = HashMap::new();
        let mut source_ranges = HashMap::new();
        let mut sparse_checkout = Vec::new();
        let mut interpolated_source_refs: HashMap<String, NixNode> = HashMap::new();

        for child in src_arg.children() {
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
                    let range = value.text_range();
                    source_ranges.insert(key.clone(), range);

                    if let Some(content) = value.pure_string_content() {
                        params.insert(key.clone(), content);
                    } else if is_recursive && (key == "tag" || key == "rev" || key == "ref") {
                        let mut vars = HashMap::new();
                        vars.insert("version".to_string(), version_content.clone());
                        if value.interpolated_string_content(&vars).is_some() {
                            interpolated_source_refs.insert(key.clone(), value);
                        }
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

        let source_ref_key = preferred_ref_key(&params)
            .map(|k| k.to_string())
            .or_else(|| {
                if interpolated_source_refs.contains_key("tag") {
                    Some("tag".to_string())
                } else if interpolated_source_refs.contains_key("rev") {
                    Some("rev".to_string())
                } else if interpolated_source_refs.contains_key("ref") {
                    Some("ref".to_string())
                } else {
                    None
                }
            });

        let source_ref_value = if let Some(key) = &source_ref_key {
            if let Some(value) = params.get(key) {
                SourceRefValue::Pure(value.clone())
            } else if let Some(template_node) = interpolated_source_refs.remove(key) {
                SourceRefValue::InterpolatedFromVersion { template_node }
            } else {
                SourceRefValue::Missing
            }
        } else {
            SourceRefValue::Missing
        };

        let source_ref_range = source_ref_key
            .as_ref()
            .and_then(|key| source_ranges.get(key))
            .copied();

        let pinned = arg.has_pin_comment()
            || node.has_pin_comment()
            || src_arg.has_pin_comment()
            || src_value.has_pin_comment();

        Some(MkDerivationCall {
            version_value: version_content,
            version_range: version_node.text_range(),
            source_ref_key,
            source_ref_value,
            source_ref_range,
            fetcher_kind,
            fetcher_params: params,
            fetcher_source_ranges: source_ranges,
            fetcher_sparse_checkout: sparse_checkout,
            pinned,
        })
    }

    fn compute_hash(
        kind: &FetcherKind,
        params: &HashMap<String, String>,
        rev: &str,
        sparse_checkout: &[String],
    ) -> Result<NarHash> {
        let has_sparse_checkout = !sparse_checkout.is_empty();
        match kind.hash_strategy(params, has_sparse_checkout) {
            HashStrategy::Tarball => tarball::compute_hash(kind, params, rev),
            HashStrategy::Git => git_fetch::compute_hash(kind, params, rev, sparse_checkout),
            HashStrategy::None => anyhow::bail!("No hash needed for this fetcher"),
        }
    }

    fn extract_version_from_interpolated_ref(
        template_node: &NixNode,
        resolved_ref: &str,
    ) -> Option<String> {
        let (prefix, suffix) = template_node.interpolated_single_var_affixes("version")?;
        if !resolved_ref.starts_with(&prefix) || !resolved_ref.ends_with(&suffix) {
            return None;
        }
        let middle = &resolved_ref[prefix.len()..resolved_ref.len() - suffix.len()];
        if middle.is_empty() {
            return None;
        }
        Some(middle.to_string())
    }

    fn check_mk_derivation_call(call: &MkDerivationCall) -> Result<Option<Vec<Update>>> {
        if call.pinned {
            return Ok(None);
        }

        let git_url = match call.fetcher_kind.git_url(&call.fetcher_params) {
            Some(url) => url,
            None => return Ok(None),
        };

        let mut updates = Vec::new();
        let mut effective_ref_changed = false;
        let mut target_version = call.version_value.clone();
        let mut new_source_ref_text: Option<String> = None;

        match &call.source_ref_value {
            SourceRefValue::Pure(current_ref) if !current_ref.is_empty() => {
                if !is_commit_hash(current_ref)
                    && VersionDetector::is_version(current_ref)
                    && current_ref == &call.version_value
                {
                    if let Some(latest) =
                        GitFetcher::get_latest_tag_matching(&git_url, Some(current_ref))?
                        && VersionDetector::compare(current_ref, &latest)
                            == std::cmp::Ordering::Less
                    {
                        target_version = latest.clone();
                        new_source_ref_text = Some(latest);
                    }
                } else if is_commit_hash(current_ref)
                    && let Some(latest) =
                        GitFetcher::get_latest_tag_matching(&git_url, Some(&call.version_value))?
                    && VersionDetector::compare(&call.version_value, &latest)
                        == std::cmp::Ordering::Less
                {
                    target_version = latest.clone();
                    new_source_ref_text = GitFetcher::resolve_ref_to_sha(&git_url, &latest)
                        .ok()
                        .flatten();
                }
            }
            SourceRefValue::Pure(current_ref) => {
                if let Some(latest) =
                    GitFetcher::get_latest_tag_matching(&git_url, Some(&call.version_value))?
                    && VersionDetector::compare(&call.version_value, &latest)
                        == std::cmp::Ordering::Less
                {
                    target_version = latest.clone();
                    new_source_ref_text = Some(latest);
                } else if current_ref.is_empty() {
                    new_source_ref_text = Some(call.version_value.clone());
                }
            }
            SourceRefValue::InterpolatedFromVersion { template_node } => {
                let mut vars = HashMap::new();
                vars.insert("version".to_string(), call.version_value.clone());
                if let Some(resolved_ref) = template_node.interpolated_string_content(&vars)
                    && let Some(latest_ref) =
                        GitFetcher::get_latest_tag_matching(&git_url, Some(&resolved_ref))?
                    && VersionDetector::compare(&resolved_ref, &latest_ref)
                        == std::cmp::Ordering::Less
                    && let Some(candidate_version) =
                        Self::extract_version_from_interpolated_ref(template_node, &latest_ref)
                    && VersionDetector::is_version(&candidate_version)
                    && VersionDetector::compare(&call.version_value, &candidate_version)
                        == std::cmp::Ordering::Less
                {
                    target_version = candidate_version;
                    effective_ref_changed = true;
                }
            }
            SourceRefValue::Missing => {}
        }

        let version_updated = VersionDetector::compare(&call.version_value, &target_version)
            == std::cmp::Ordering::Less;
        if version_updated {
            updates.push(Update::new(
                "mkDerivation.version",
                format!("\"{}\"", target_version),
                call.version_range,
            ));
        }

        if let (Some(key), Some(range), Some(new_ref_text)) = (
            call.source_ref_key.as_ref(),
            call.source_ref_range,
            new_source_ref_text.as_ref(),
        ) && let SourceRefValue::Pure(old_ref_text) = &call.source_ref_value
            && old_ref_text != new_ref_text
        {
            updates.push(Update::new(
                format!("{}.{}", call.fetcher_kind.name(), key),
                format!("\"{}\"", new_ref_text),
                range,
            ));
            effective_ref_changed = true;
        }

        let hash_empty = call
            .fetcher_params
            .get("hash")
            .is_some_and(String::is_empty)
            || call
                .fetcher_params
                .get("sha256")
                .is_some_and(String::is_empty);
        let should_refresh_hash =
            call.fetcher_kind.needs_hash() && (hash_empty || effective_ref_changed);
        if should_refresh_hash {
            let rev_for_hash = if let Some(new_ref_text) = new_source_ref_text.as_ref() {
                resolve_ref_for_prefetch(&git_url, new_ref_text)
            } else {
                match &call.source_ref_value {
                    SourceRefValue::Pure(reference) => {
                        resolve_ref_for_prefetch(&git_url, reference)
                    }
                    SourceRefValue::InterpolatedFromVersion { template_node } => {
                        let mut vars = HashMap::new();
                        vars.insert("version".to_string(), target_version.clone());
                        template_node
                            .interpolated_string_content(&vars)
                            .and_then(|resolved| resolve_ref_for_prefetch(&git_url, &resolved))
                    }
                    SourceRefValue::Missing => None,
                }
            };

            if let Some(rev_for_hash) = rev_for_hash {
                let result = Self::compute_hash(
                    &call.fetcher_kind,
                    &call.fetcher_params,
                    &rev_for_hash,
                    &call.fetcher_sparse_checkout,
                );
                match result {
                    Ok(nar_hash) => {
                        if let Some(range) = call.fetcher_source_ranges.get("hash") {
                            updates.push(Update::new(
                                format!("{}.hash", call.fetcher_kind.name()),
                                format!("\"{}\"", nar_hash.sri),
                                *range,
                            ));
                        }
                        if let Some(range) = call.fetcher_source_ranges.get("sha256") {
                            updates.push(Update::new(
                                format!("{}.sha256", call.fetcher_kind.name()),
                                format!("\"{}\"", nar_hash.nix32),
                                *range,
                            ));
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: could not prefetch hash for {} @ {}: {}",
                            git_url, rev_for_hash, e
                        );
                    }
                }
            }
        }

        if updates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(updates))
        }
    }
}

impl Default for MkDerivationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateRule for MkDerivationRule {
    fn name(&self) -> &str {
        "mk-derivation"
    }

    fn matches(&self, node: &NixNode) -> bool {
        node.kind() == rnix::SyntaxKind::NODE_APPLY
    }

    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>> {
        let call = match Self::try_extract_call(node) {
            Some(call) => call,
            None => return Ok(None),
        };
        Self::check_mk_derivation_call(&call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_commit_hash_valid() {
        assert!(is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7ba"));
        assert!(is_commit_hash("0000000000000000000000000000000000000000"));
    }

    #[test]
    fn test_is_commit_hash_invalid() {
        assert!(!is_commit_hash("v1.0.0"));
        assert!(!is_commit_hash("main"));
        assert!(!is_commit_hash("short"));
        assert!(!is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7ba0"));
        assert!(!is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7b"));
    }
}
