//! Per-fetcher-category check dispatch: given an extracted `FetcherCall`,
//! decide what changed (version bump, `# follow:` resolution) and what
//! needs rehashing.

use crate::rules::traits::{CheckResult, CheckWarning, Update, UpdateGroup};
use crate::utils::{GitFetcher, VersionDetector};

use super::extract::FetcherCall;
use super::follow::{FollowSpec, parse_follow_spec, resolve_follow};
use super::hashing::{try_prefetch_empty_hash, try_prefetch_hash};
use super::kind::FetcherKind;
use super::{source_url, version_ref_key_and_value};

/// The attr key that carries the commit-ish for `call`: `rev` when present,
/// otherwise `ref` for `builtins.fetchGit`, else `rev` (the common default).
fn ref_key(call: &FetcherCall) -> &'static str {
    if call.parsed().strings.contains_key("rev") {
        "rev"
    } else if call.kind() == FetcherKind::BuiltinsFetchGit {
        "ref"
    } else {
        "rev"
    }
}

fn handle_following(
    call: &FetcherCall,
    git_url: &str,
    spec: &FollowSpec,
) -> (Option<String>, Vec<CheckWarning>) {
    let (result, ws) = resolve_follow(spec, git_url);
    let Some(result) = result else {
        return (None, ws);
    };

    let new_sha = result.sha;

    let current_ref = call
        .parsed()
        .strings
        .get("rev")
        .or_else(|| call.parsed().strings.get("ref"));

    if let Some(current) = current_ref
        && current == &new_sha
    {
        return (None, ws);
    }

    if call.parsed().string_range(ref_key(call)).is_some() {
        (Some(new_sha), ws)
    } else {
        (None, ws)
    }
}

fn handle_version_update(call: &FetcherCall, git_url: &str) -> (Option<String>, Vec<CheckWarning>) {
    let Some((_version_key, current_version)) =
        version_ref_key_and_value(call.kind(), call.parsed())
    else {
        return (None, vec![]);
    };

    let latest = match GitFetcher::get_latest_tag_matching(git_url, Some(&current_version)) {
        Ok(Some(tag)) => tag,
        Ok(None) => return (None, vec![]),
        Err(e) => {
            return (
                None,
                vec![CheckWarning::FollowResolutionFailed {
                    git_url: git_url.to_string(),
                    source: e,
                }],
            );
        }
    };

    if VersionDetector::compare(&current_version, &latest) != std::cmp::Ordering::Less {
        return (None, vec![]);
    }

    (Some(latest), vec![])
}

pub(super) fn check_fetcher_call(call: &FetcherCall) -> CheckResult {
    let Some(git_url) = call.kind().git_url(call.parsed()) else {
        return CheckResult::empty();
    };

    let mut updates = Vec::new();
    let mut warnings = Vec::new();
    let mut version_updated_rev: Option<String> = None;
    let mut hash_failed = false;
    if !call.pinned() {
        if let Some(follow_str) = call.follow() {
            let (spec, ws) = parse_follow_spec(follow_str);
            warnings.extend(ws);
            if let Some(spec) = spec {
                let (new_sha, ws) = handle_following(call, &git_url, &spec);
                warnings.extend(ws);
                if let Some(sha) = &new_sha
                    && let Some(range) = call.parsed().string_range(ref_key(call))
                {
                    updates.push(Update::new(
                        format!("{}.rev", call.kind().name()),
                        format!("\"{}\"", sha),
                        range,
                    ));
                }
                version_updated_rev = new_sha;
            }
        } else {
            let (new_version, ws) = handle_version_update(call, &git_url);
            warnings.extend(ws);
            if let Some(version) = &new_version
                && let Some((version_key, _)) =
                    version_ref_key_and_value(call.kind(), call.parsed())
                && let Some(range) = call.parsed().string_range(version_key)
            {
                updates.push(Update::new(
                    format!("{}.{}", call.kind().name(), version_key),
                    format!("\"{}\"", version),
                    range,
                ));
            }
            version_updated_rev = new_version;
        }
    }

    let needs_hash = call.kind().needs_hash();
    if needs_hash {
        let (ok, ws) = if let Some(rev) = &version_updated_rev {
            try_prefetch_hash(call, rev, &mut updates)
        } else {
            try_prefetch_empty_hash(call, &git_url, &mut updates)
        };
        warnings.extend(ws);
        if !ok {
            hash_failed = true;
        }
    }

    if hash_failed {
        CheckResult::with_warnings(warnings)
    } else if updates.is_empty() {
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

pub(super) fn check_fetchpatch_call(call: &FetcherCall) -> CheckResult {
    let url = match call.parsed().strings.get("url") {
        Some(url) => url.clone(),
        None => match call.parsed().pure_string_list("urls") {
            Some(urls) if !urls.is_empty() => urls[0].clone(),
            _ => return CheckResult::empty(),
        },
    };

    let mut updates = Vec::new();
    let mut warnings = Vec::new();
    let mut current_url = url.clone();
    let mut url_changed = false;
    let mut hash_failed = false;

    let parsed_url = source_url::parse_patch_url(&url);

    if !call.pinned() {
        if let Some(follow_str) = call.follow() {
            let (spec, ws) = parse_follow_spec(follow_str);
            warnings.extend(ws);
            if let Some(spec) = spec
                && let Some(parsed) = &parsed_url
            {
                let git_url = parsed.git_remote_url();
                let (result, ws) = resolve_follow(&spec, &git_url);
                warnings.extend(ws);
                if let Some(result) = result {
                    let current_ref = parsed.current_ref();
                    if current_ref != result.sha {
                        current_url = parsed.replace_ref(&result.sha);
                        url_changed = true;
                    }
                }
            }
        } else if let Some(parsed) = &parsed_url
            && parsed.is_version_ref()
        {
            let git_url = parsed.git_remote_url();
            let current = parsed.current_ref();
            match GitFetcher::get_latest_tag_matching(&git_url, Some(current)) {
                Ok(Some(latest))
                    if VersionDetector::compare(current, &latest) == std::cmp::Ordering::Less =>
                {
                    current_url = parsed.replace_ref(&latest);
                    url_changed = true;
                }
                Ok(_) => {}
                Err(e) => {
                    warnings.push(CheckWarning::FollowResolutionFailed {
                        git_url: git_url.clone(),
                        source: e,
                    });
                }
            }
        }
    }

    if url_changed && let Some(range) = call.parsed().string_range("url") {
        updates.push(Update::new(
            format!("{}.url", call.kind().name()),
            format!("\"{}\"", current_url),
            range,
        ));
    }

    let strip_len: usize = call
        .parsed()
        .ints
        .get("stripLen")
        .copied()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);

    let relative = call.parsed().strings.get("relative").cloned();
    let extra_prefix = call.parsed().strings.get("extraPrefix").cloned();
    let revert = call.parsed().bools.get("revert").copied().unwrap_or(false);

    let has_post_fetch = call
        .parsed()
        .strings
        .get("postFetch")
        .is_some_and(|p| !p.is_empty());

    let has_curl_opts = call
        .parsed()
        .strings
        .get("curlOpts")
        .is_some_and(|o| !o.is_empty());

    let has_curl_opts_list = call
        .parsed()
        .pure_string_list("curlOptsList")
        .is_some_and(|v| !v.is_empty());

    let has_netrc_phase = call
        .parsed()
        .strings
        .get("netrcPhase")
        .is_some_and(|p| !p.is_empty());

    let has_netrc_impure_env_vars = call
        .parsed()
        .pure_string_list("netrcImpureEnvVars")
        .is_some_and(|v| !v.is_empty());

    let has_recursive_hash = call.parsed().bools.get("recursiveHash").is_some_and(|&v| v);

    let has_show_urls = call.parsed().bools.get("showURLs").is_some_and(|&v| v);

    let has_non_sha256_hash_algo = call
        .parsed()
        .strings
        .get("sha1")
        .is_some_and(|h| !h.is_empty())
        || call
            .parsed()
            .strings
            .get("sha512")
            .is_some_and(|h| !h.is_empty())
        || call
            .parsed()
            .strings
            .get("outputHashAlgo")
            .is_some_and(|a| a != "sha256");

    let decode = call
        .parsed()
        .strings
        .get("decode")
        .cloned()
        .unwrap_or_else(|| "cat".to_string());
    let can_decode = decode == "cat";

    let needs_hash = (url_changed
        || call
            .parsed()
            .strings
            .get("hash")
            .is_some_and(std::string::String::is_empty)
        || call
            .parsed()
            .strings
            .get("sha256")
            .is_some_and(std::string::String::is_empty)
        || call
            .parsed()
            .strings
            .get("outputHash")
            .is_some_and(std::string::String::is_empty))
        && !has_post_fetch
        && !has_curl_opts
        && !has_curl_opts_list
        && !has_netrc_phase
        && !has_netrc_impure_env_vars
        && !has_recursive_hash
        && !has_show_urls
        && !has_non_sha256_hash_algo
        && can_decode;

    if needs_hash {
        let has_hash_source = call.parsed().has_string("hash")
            || call.parsed().has_string("sha256")
            || call.parsed().has_string("outputHash");

        if has_hash_source {
            let options = crate::utils::PatchOptions {
                strip_len,
                relative,
                extra_prefix,
                excludes: call
                    .parsed()
                    .pure_string_list("excludes")
                    .unwrap_or_default(),
                includes: call
                    .parsed()
                    .pure_string_list("includes")
                    .unwrap_or_default(),
                hunks: call
                    .parsed()
                    .list_ints
                    .get("hunks")
                    .map(|v| v.iter().filter_map(|&i| usize::try_from(i).ok()).collect())
                    .unwrap_or_default(),
                revert,
            };
            let result = crate::utils::PatchHasher::hash_patch_url(&current_url, &options);
            match result {
                Ok(nar_hash) => {
                    if let Some(range) = call.parsed().string_range("hash") {
                        updates.push(Update::new(
                            format!("{}.hash", call.kind().name()),
                            format!("\"{}\"", nar_hash.sri),
                            range,
                        ));
                    }
                    if let Some(range) = call.parsed().string_range("sha256") {
                        updates.push(Update::new(
                            format!("{}.sha256", call.kind().name()),
                            format!("\"{}\"", nar_hash.nix32),
                            range,
                        ));
                    }
                    if let Some(range) = call.parsed().string_range("outputHash") {
                        updates.push(Update::new(
                            format!("{}.outputHash", call.kind().name()),
                            format!("\"{}\"", nar_hash.sri),
                            range,
                        ));
                    }
                }
                Err(e) => {
                    warnings.push(CheckWarning::HashPrefetchFailed {
                        url: current_url,
                        rev: String::new(),
                        source: e,
                    });
                    hash_failed = true;
                }
            }
        }
    }

    if hash_failed {
        CheckResult::with_warnings(warnings)
    } else if updates.is_empty() {
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

pub(super) fn check_fetchtarball_call(call: &FetcherCall) -> CheckResult {
    let url = match call.parsed().strings.get("url") {
        Some(url) => url.clone(),
        None => match call.parsed().pure_string_list("urls") {
            Some(urls) if !urls.is_empty() => urls[0].clone(),
            _ => return CheckResult::empty(),
        },
    };

    let mut updates = Vec::new();
    let mut warnings = Vec::new();
    let mut current_url = url.clone();
    let mut url_changed = false;
    let mut hash_failed = false;

    let parsed_url = source_url::parse_source_url(&url);

    if !call.pinned() {
        if let Some(follow_str) = call.follow() {
            let (spec, ws) = parse_follow_spec(follow_str);
            warnings.extend(ws);
            if let Some(spec) = spec
                && let Some(parsed) = &parsed_url
            {
                let git_url = parsed.git_remote_url();
                let (result, ws) = resolve_follow(&spec, &git_url);
                warnings.extend(ws);
                if let Some(result) = result {
                    let current_ref = parsed.current_ref();
                    if current_ref != result.sha {
                        current_url = parsed.replace_ref(&result.sha);
                        url_changed = true;
                    }
                }
            }
        } else if let Some(parsed) = &parsed_url
            && parsed.is_version_ref()
        {
            let git_url = parsed.git_remote_url();
            let current = parsed.current_ref();
            match GitFetcher::get_latest_tag_matching(&git_url, Some(current)) {
                Ok(Some(latest))
                    if VersionDetector::compare(current, &latest) == std::cmp::Ordering::Less =>
                {
                    current_url = parsed.replace_ref(&latest);
                    url_changed = true;
                }
                Ok(_) => {}
                Err(e) => {
                    warnings.push(CheckWarning::FollowResolutionFailed {
                        git_url: git_url.clone(),
                        source: e,
                    });
                }
            }
        }
    }

    if url_changed && let Some(range) = call.parsed().string_range("url") {
        updates.push(Update::new(
            format!("{}.url", call.kind().name()),
            format!("\"{}\"", current_url),
            range,
        ));
    }

    let needs_hash = (url_changed
        || call
            .parsed()
            .strings
            .get("hash")
            .is_some_and(std::string::String::is_empty)
        || call
            .parsed()
            .strings
            .get("sha256")
            .is_some_and(std::string::String::is_empty))
        && call.kind().needs_hash();

    if needs_hash {
        let has_hash_source =
            call.parsed().has_string("hash") || call.parsed().has_string("sha256");

        if has_hash_source {
            let result = crate::utils::TarballHasher::hash_tarball_url(&current_url);
            match result {
                Ok(nar_hash) => {
                    if let Some(range) = call.parsed().string_range("hash") {
                        updates.push(Update::new(
                            format!("{}.hash", call.kind().name()),
                            format!("\"{}\"", nar_hash.sri),
                            range,
                        ));
                    }
                    if let Some(range) = call.parsed().string_range("sha256") {
                        updates.push(Update::new(
                            format!("{}.sha256", call.kind().name()),
                            format!("\"{}\"", nar_hash.nix32),
                            range,
                        ));
                    }
                }
                Err(e) => {
                    warnings.push(CheckWarning::HashPrefetchFailed {
                        url: current_url,
                        rev: String::new(),
                        source: e,
                    });
                    hash_failed = true;
                }
            }
        }
    }

    if hash_failed {
        CheckResult::with_warnings(warnings)
    } else if updates.is_empty() {
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
