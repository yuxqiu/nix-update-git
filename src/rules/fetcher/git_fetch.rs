use std::collections::HashMap;

use anyhow::Result;

use crate::utils::{NarHash, NixPrefetcher};

use super::kind::FetcherKind;

pub fn compute_hash(
    kind: &FetcherKind,
    params: &HashMap<String, String>,
    rev: &str,
) -> Result<NarHash> {
    let git_url = match kind.git_url(params) {
        Some(url) => url,
        None => anyhow::bail!("No git URL available"),
    };
    let use_submodules = kind.uses_fetch_submodules(params);
    let prefetch = if use_submodules {
        NixPrefetcher::prefetch_git_with_submodules(&git_url, rev)?
    } else {
        NixPrefetcher::prefetch_git(&git_url, rev)?
    };
    Ok(NarHash {
        sri: prefetch.sri_hash,
        nix32: prefetch.sha256_nix,
    })
}
