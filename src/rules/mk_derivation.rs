use std::collections::HashMap;

use anyhow::Result;

use crate::parser::{NixNode, TextRange};
use crate::rules::fetcher::{git_fetch, kind::FetcherKind, kind::HashStrategy, tarball};
use crate::rules::traits::{Update, UpdateRule};
use crate::utils::{GitFetcher, NarHash, VersionDetector};

// TODO: Future improvements:
// - Detect `name = "foo-${version}"` patterns inside `rec` attr sets. When
//   `name` uses `${version}` interpolation, updating `version` alone is correct
//   since `name` references it dynamically. Consider warning or suggesting
//   an update when `name` embeds the version literally (e.g. `name =
//   "foo-1.0.0"`) rather than via interpolation.
// - Support `pname` alongside `name` — the `pname` attribute is commonly used
//   in modern nixpkgs and should be treated similarly to `name` for
//   identification purposes.

struct MkDerivationCall {
    version_value: String,
    version_range: TextRange,
    fetcher_kind: FetcherKind,
    fetcher_params: HashMap<String, String>,
    fetcher_source_ranges: HashMap<String, TextRange>,
    fetcher_sparse_checkout: Vec<String>,
    pinned: bool,
}

fn is_commit_hash(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
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

        let rev = params.get("rev")?;
        if !is_commit_hash(rev) {
            return None;
        }

        let pinned = arg.has_pin_comment()
            || node.has_pin_comment()
            || src_arg.has_pin_comment()
            || src_value.has_pin_comment();

        Some(MkDerivationCall {
            version_value: version_content,
            version_range: version_node.text_range(),
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

    fn check_mk_derivation_call(call: &MkDerivationCall) -> Result<Option<Vec<Update>>> {
        if call.pinned {
            return Ok(None);
        }

        let git_url = match call.fetcher_kind.git_url(&call.fetcher_params) {
            Some(url) => url,
            None => return Ok(None),
        };

        let latest = match GitFetcher::get_latest_tag_matching(&git_url, Some(&call.version_value))?
        {
            Some(tag) => tag,
            None => return Ok(None),
        };

        if VersionDetector::compare(&call.version_value, &latest) != std::cmp::Ordering::Less {
            return Ok(None);
        }

        let new_rev = match GitFetcher::resolve_ref_to_sha(&git_url, &latest)
            .ok()
            .flatten()
        {
            Some(sha) => sha,
            None => return Ok(None),
        };

        let mut updates = Vec::new();

        updates.push(Update::new(
            "mkDerivation.version",
            format!("\"{}\"", latest),
            call.version_range,
        ));

        if let Some(range) = call.fetcher_source_ranges.get("rev") {
            updates.push(Update::new(
                format!("{}.rev", call.fetcher_kind.name()),
                format!("\"{}\"", new_rev),
                *range,
            ));
        }

        if call.fetcher_kind.needs_hash() {
            let result = Self::compute_hash(
                &call.fetcher_kind,
                &call.fetcher_params,
                &new_rev,
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
                        git_url, new_rev, e
                    );
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
