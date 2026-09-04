//! Source-ref precedence resolution: given an extracted `DerivationCall`,
//! decide the new version/ref/hash (if any) and produce the updates.

use std::collections::HashMap;

use crate::parser::NixNode;
use crate::rules::fetcher::{hashing::push_hash_updates, is_commit_hash, resolve_ref_for_prefetch};
use crate::rules::traits::{CheckResult, CheckWarning, Update, UpdateGroup};
use crate::utils::{GitFetcher, VersionDetector};

use super::extract::{DerivationCall, SourceRefValue};
use super::hashing::compute_hash;

fn extract_version_from_interpolated_ref(
    template_node: &NixNode,
    resolved_ref: &str,
    version_var: &str,
    vars: &HashMap<String, String>,
) -> Option<String> {
    let (prefix, suffix) = template_node.interpolated_var_affixes(version_var, vars)?;
    if !resolved_ref.starts_with(&prefix) || !resolved_ref.ends_with(&suffix) {
        return None;
    }
    let middle = &resolved_ref[prefix.len()..resolved_ref.len() - suffix.len()];
    if middle.is_empty() {
        return None;
    }
    Some(middle.to_string())
}

pub(super) fn check_derivation_call(rule_name: &str, call: &DerivationCall) -> CheckResult {
    if call.pinned() {
        return CheckResult::empty();
    }

    let git_url = match call.fetcher_kind().git_url(call.fetcher_parsed()) {
        Some(url) => url,
        None => return CheckResult::empty(),
    };

    let mut updates = Vec::new();
    let mut warnings: Vec<CheckWarning> = Vec::new();
    let mut effective_ref_changed = false;
    let mut target_version = call.version_value().to_string();
    let mut new_source_ref_text: Option<String> = None;

    match call.source_ref_value() {
        SourceRefValue::Pure(current_ref) if !current_ref.is_empty() => {
            if !is_commit_hash(current_ref)
                && VersionDetector::is_version(current_ref)
                && current_ref == call.version_value()
            {
                match GitFetcher::get_latest_tag_matching(&git_url, Some(current_ref)) {
                    Ok(Some(latest))
                        if VersionDetector::compare(current_ref, &latest)
                            == std::cmp::Ordering::Less =>
                    {
                        target_version = latest.clone();
                        new_source_ref_text = Some(latest);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(CheckWarning::FollowResolutionFailed {
                            git_url: git_url.clone(),
                            source: e,
                        });
                        return CheckResult::with_warnings(warnings);
                    }
                }
            } else if is_commit_hash(current_ref) {
                match GitFetcher::get_latest_tag_matching(&git_url, Some(call.version_value())) {
                    Ok(Some(latest))
                        if VersionDetector::compare(call.version_value(), &latest)
                            == std::cmp::Ordering::Less =>
                    {
                        target_version = latest.clone();
                        new_source_ref_text = GitFetcher::resolve_ref_to_sha(&git_url, &latest)
                            .ok()
                            .flatten();
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(CheckWarning::FollowResolutionFailed {
                            git_url: git_url.clone(),
                            source: e,
                        });
                        return CheckResult::with_warnings(warnings);
                    }
                }
            }
        }
        SourceRefValue::Pure(current_ref) => {
            match GitFetcher::get_latest_tag_matching(&git_url, Some(call.version_value())) {
                Ok(Some(latest))
                    if VersionDetector::compare(call.version_value(), &latest)
                        == std::cmp::Ordering::Less =>
                {
                    target_version = latest.clone();
                    new_source_ref_text = Some(latest);
                }
                Ok(_) => {
                    if current_ref.is_empty() {
                        new_source_ref_text = Some(call.version_value().to_string());
                    }
                }
                Err(e) => {
                    warnings.push(CheckWarning::FollowResolutionFailed {
                        git_url: git_url.clone(),
                        source: e,
                    });
                    return CheckResult::with_warnings(warnings);
                }
            }
        }
        SourceRefValue::InterpolatedFromVersion {
            template_node,
            version_var,
        } => {
            let mut vars = call.extra_vars().clone();
            vars.insert(version_var.clone(), call.version_value().to_string());
            if let Some(resolved_ref) = template_node.interpolated_string_content(&vars) {
                match GitFetcher::get_latest_tag_matching(&git_url, Some(&resolved_ref)) {
                    Ok(Some(latest_ref))
                        if VersionDetector::compare(&resolved_ref, &latest_ref)
                            == std::cmp::Ordering::Less =>
                    {
                        if let Some(candidate_version) = extract_version_from_interpolated_ref(
                            template_node,
                            &latest_ref,
                            version_var,
                            call.extra_vars(),
                        ) && VersionDetector::is_version(&candidate_version)
                            && VersionDetector::compare(call.version_value(), &candidate_version)
                                == std::cmp::Ordering::Less
                        {
                            target_version = candidate_version;
                            effective_ref_changed = true;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(CheckWarning::FollowResolutionFailed {
                            git_url: git_url.clone(),
                            source: e,
                        });
                        return CheckResult::with_warnings(warnings);
                    }
                }
            }
        }
        SourceRefValue::IdentFromVersion => {
            match GitFetcher::get_latest_tag_matching(&git_url, Some(call.version_value())) {
                Ok(Some(latest))
                    if VersionDetector::compare(call.version_value(), &latest)
                        == std::cmp::Ordering::Less =>
                {
                    target_version = latest;
                    effective_ref_changed = true;
                }
                Ok(_) => {}
                Err(e) => {
                    warnings.push(CheckWarning::FollowResolutionFailed {
                        git_url: git_url.clone(),
                        source: e,
                    });
                    return CheckResult::with_warnings(warnings);
                }
            }
        }
        SourceRefValue::Missing => {}
    }

    let version_updated =
        VersionDetector::compare(call.version_value(), &target_version) == std::cmp::Ordering::Less;
    if version_updated {
        updates.push(Update::new(
            format!("{}.version", rule_name),
            format!("\"{}\"", target_version),
            call.version_range(),
        ));
    }

    if let (Some(key), Some(range), Some(new_ref_text)) = (
        call.source_ref_key().as_ref(),
        call.source_ref_range(),
        new_source_ref_text.as_ref(),
    ) && let SourceRefValue::Pure(old_ref_text) = call.source_ref_value()
        && old_ref_text != new_ref_text
    {
        updates.push(Update::new(
            format!("{}.{}", call.fetcher_kind().name(), key),
            format!("\"{}\"", new_ref_text),
            range,
        ));
        effective_ref_changed = true;
    }

    let hash_empty = call
        .fetcher_parsed()
        .strings
        .get("hash")
        .is_some_and(String::is_empty)
        || call
            .fetcher_parsed()
            .strings
            .get("sha256")
            .is_some_and(String::is_empty);
    let should_refresh_hash =
        call.fetcher_kind().needs_hash() && (hash_empty || effective_ref_changed);
    let mut hash_failed = false;
    if should_refresh_hash {
        let rev_for_hash = if let Some(new_ref_text) = new_source_ref_text.as_ref() {
            resolve_ref_for_prefetch(&git_url, new_ref_text)
        } else {
            match call.source_ref_value() {
                SourceRefValue::Pure(reference) => resolve_ref_for_prefetch(&git_url, reference),
                SourceRefValue::InterpolatedFromVersion {
                    template_node,
                    version_var,
                } => {
                    let mut vars = call.extra_vars().clone();
                    vars.insert(version_var.clone(), target_version.clone());
                    template_node
                        .interpolated_string_content(&vars)
                        .and_then(|resolved| resolve_ref_for_prefetch(&git_url, &resolved))
                }
                SourceRefValue::IdentFromVersion => {
                    resolve_ref_for_prefetch(&git_url, &target_version)
                }
                SourceRefValue::Missing => None,
            }
        };

        if let Some(rev_for_hash) = rev_for_hash {
            let result = compute_hash(&call.fetcher_kind(), call.fetcher_parsed(), &rev_for_hash);
            match result {
                Ok(nar_hash) => {
                    push_hash_updates(
                        call.fetcher_parsed(),
                        call.fetcher_kind().name(),
                        &nar_hash,
                        &mut updates,
                    );
                }
                Err(e) => {
                    warnings.push(CheckWarning::HashPrefetchFailed {
                        url: git_url.clone(),
                        rev: rev_for_hash.clone(),
                        source: e,
                    });
                    hash_failed = true;
                }
            }
        } else {
            warnings.push(CheckWarning::HashPrefetchFailed {
                url: git_url.clone(),
                rev: String::new(),
                source: anyhow::anyhow!("could not resolve ref for hash computation"),
            });
            hash_failed = true;
        }
    }

    if hash_failed {
        return CheckResult::with_warnings(warnings);
    }

    if updates.is_empty() {
        CheckResult {
            groups: vec![],
            warnings,
        }
    } else {
        CheckResult {
            groups: vec![UpdateGroup::new(updates)],
            warnings,
        }
    }
}
