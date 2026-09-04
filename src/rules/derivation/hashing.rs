//! Hash computation for derivation-rule call sites: delegates the actual
//! tarball-vs-git dispatch to `rules::fetcher::hashing`, which the fetcher
//! rule uses too.

use anyhow::Result;

use crate::parser::ParsedAttrs;
use crate::rules::fetcher::{hashing::dispatch_hash_strategy, kind::FetcherKind};
use crate::utils::NarHash;

pub(super) fn compute_hash(kind: &FetcherKind, parsed: &ParsedAttrs, rev: &str) -> Result<NarHash> {
    dispatch_hash_strategy(kind, parsed, rev)
}
