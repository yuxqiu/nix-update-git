//! Hash prefetching: resolving which rev to hash, computing it via the
//! right strategy (tarball vs. git clone), and turning the result into
//! `hash`/`sha256` updates.

use crate::parser::ParsedAttrs;
use crate::rules::traits::{CheckWarning, Update};
use crate::utils::NarHash;

use super::extract::FetcherCall;
use super::kind::{FetcherKind, HashStrategy};
use super::{git_fetch, preferred_ref_key, resolve_ref_for_prefetch, tarball};

fn resolve_rev(call: &FetcherCall, git_url: &str) -> Option<String> {
    let key = preferred_ref_key(call.parsed())?;
    let ref_value = call.parsed().strings.get(key)?;
    resolve_ref_for_prefetch(git_url, ref_value)
}

/// Dispatches to the right hashing strategy (tarball vs. git clone) for
/// `kind`. Shared by the fetcher rule (via `compute_hash` below) and by
/// `rules::derivation::hashing::compute_hash`, which mirrors the same
/// dispatch for derivation-rule call sites.
pub(crate) fn dispatch_hash_strategy(
    kind: &FetcherKind,
    parsed: &ParsedAttrs,
    rev: &str,
) -> anyhow::Result<NarHash> {
    let has_sparse_checkout = parsed
        .pure_string_list("sparseCheckout")
        .is_some_and(|v| !v.is_empty());
    match kind.hash_strategy(parsed, has_sparse_checkout) {
        HashStrategy::Tarball => tarball::compute_hash(kind, parsed, rev),
        HashStrategy::Git => {
            let sparse_checkout = parsed
                .pure_string_list("sparseCheckout")
                .unwrap_or_default();
            git_fetch::compute_hash(kind, parsed, rev, &sparse_checkout)
        }
        HashStrategy::Patch => {
            anyhow::bail!("Patch hashing should be handled via check_fetchpatch_call")
        }
        HashStrategy::None => {
            anyhow::bail!("No hash needed for this fetcher")
        }
    }
}

/// Pushes `hash`/`sha256` `Update`s for whichever of those fields is present
/// in `parsed`. Shared by the fetcher rule (below) and
/// `rules::derivation::resolve`, which produces the same pair of updates
/// from its own `NarHash` result.
pub(crate) fn push_hash_updates(
    parsed: &ParsedAttrs,
    kind_name: &str,
    nar_hash: &NarHash,
    updates: &mut Vec<Update>,
) {
    if let Some(range) = parsed.string_range("hash") {
        updates.push(Update::new(
            format!("{}.hash", kind_name),
            format!("\"{}\"", nar_hash.sri),
            range,
        ));
    }
    if let Some(range) = parsed.string_range("sha256") {
        updates.push(Update::new(
            format!("{}.sha256", kind_name),
            format!("\"{}\"", nar_hash.nix32),
            range,
        ));
    }
}

fn compute_hash(call: &FetcherCall, rev: &str) -> anyhow::Result<NarHash> {
    dispatch_hash_strategy(&call.kind(), call.parsed(), rev)
}

pub(super) fn try_prefetch_hash(
    call: &FetcherCall,
    rev: &str,
    updates: &mut Vec<Update>,
) -> (bool, Vec<CheckWarning>) {
    if !call.parsed().has_string("hash") && !call.parsed().has_string("sha256") {
        // No hash field to update — not a failure, just nothing to do.
        return (true, vec![]);
    }

    let result = compute_hash(call, rev);

    match result {
        Ok(nar_hash) => {
            push_hash_updates(call.parsed(), call.kind().name(), &nar_hash, updates);
            (true, vec![])
        }
        Err(e) => {
            let git_url = call.kind().git_url(call.parsed()).unwrap_or_default();
            (
                false,
                vec![CheckWarning::HashPrefetchFailed {
                    url: git_url,
                    rev: rev.to_string(),
                    source: e,
                }],
            )
        }
    }
}

pub(super) fn try_prefetch_empty_hash(
    call: &FetcherCall,
    git_url: &str,
    updates: &mut Vec<Update>,
) -> (bool, Vec<CheckWarning>) {
    let has_empty_hash = call
        .parsed()
        .strings
        .get("hash")
        .is_some_and(|h| h.is_empty())
        || call
            .parsed()
            .strings
            .get("sha256")
            .is_some_and(|h| h.is_empty());

    if !has_empty_hash {
        return (true, vec![]);
    }

    if let Some(rev) = resolve_rev(call, git_url) {
        try_prefetch_hash(call, &rev, updates)
    } else {
        (true, vec![])
    }
}
