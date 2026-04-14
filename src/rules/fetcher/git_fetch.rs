use std::collections::HashMap;

use anyhow::Result;

use crate::utils::{GitPrefetchArgs, NarHash, NixPrefetcher};

use super::kind::FetcherKind;

fn bool_param(params: &HashMap<String, String>, key: &str) -> bool {
    params.get(key).is_some_and(|v| v == "true")
}

pub fn compute_hash(
    kind: &FetcherKind,
    params: &HashMap<String, String>,
    rev: &str,
    sparse_checkout: &[String],
) -> Result<NarHash> {
    let git_url = match kind.git_url(params) {
        Some(url) => url,
        None => anyhow::bail!("No git URL available"),
    };

    let submodules_key = if kind == &FetcherKind::BuiltinsFetchGit {
        "submodules"
    } else {
        "fetchSubmodules"
    };

    let git_args = GitPrefetchArgs {
        fetch_submodules: bool_param(params, submodules_key),
        deep_clone: bool_param(params, "deepClone"),
        leave_dot_git: bool_param(params, "leaveDotGit"),
        fetch_lfs: bool_param(params, "fetchLFS"),
        branch_name: params.get("branchName").cloned(),
        root_dir: params.get("rootDir").cloned().filter(|v| !v.is_empty()),
        sparse_checkout: sparse_checkout.to_vec(),
    };

    let prefetch = NixPrefetcher::prefetch_git(&git_url, rev, &git_args)?;
    Ok(NarHash {
        sri: prefetch.sri_hash,
        nix32: prefetch.sha256_nix,
    })
}
