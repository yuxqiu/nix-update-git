//! Hash computation dispatch, mirroring `rules::fetcher::hashing` but for
//! derivation-rule call sites.

use anyhow::Result;

use crate::parser::ParsedAttrs;
use crate::rules::fetcher::{git_fetch, kind::FetcherKind, kind::HashStrategy, tarball};
use crate::utils::NarHash;

pub(super) fn compute_hash(kind: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> Result<NarHash> {
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
        HashStrategy::Patch => anyhow::bail!("Patch hashing should be handled by fetcher rule"),
        HashStrategy::None => anyhow::bail!("No hash needed for this fetcher"),
    }
}
