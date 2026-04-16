use std::collections::HashMap;

use anyhow::Result;
use nix_prefetch_git::{NarHash, PrefetchArgs, prefetch};

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

    let fetch_submodules = bool_param(params, submodules_key);

    let args = PrefetchArgs {
        url: git_url.clone(),
        rev: rev.to_string(),
        fetch_submodules,
        deep_clone: bool_param(params, "deepClone"),
        leave_dot_git: bool_param(params, "leaveDotGit"),
        fetch_lfs: bool_param(params, "fetchLFS"),
        branch_name: params.get("branchName").cloned(),
        root_dir: params.get("rootDir").cloned().filter(|v| !v.is_empty()),
        sparse_checkout: sparse_checkout.to_vec(),
    };

    let result = prefetch(&args).map_err(|e| {
        anyhow::anyhow!(
            "Failed to prefetch git repository {} @ {}{}: {e}",
            git_url,
            rev,
            if fetch_submodules {
                " (with submodules)"
            } else {
                ""
            }
        )
    })?;

    Ok(NarHash {
        sri: result.sri_hash,
        nix32: result.sha256_nix,
    })
}
