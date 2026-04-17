use std::collections::HashMap;

use anyhow::Result;
use nix_prefetch_git::{NarHash, PrefetchArgs, prefetch};

use super::kind::FetcherKind;

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

    // Pass Option<bool> for all boolean fields: None means "use nixpkgs default"
    // (the nix-prefetch-git library resolves defaults internally). This lets us
    // distinguish "not specified" from "explicitly set to false".
    let fetch_submodules = params.get(submodules_key).map(|v| v == "true");
    let deep_clone = params.get("deepClone").map(|v| v == "true");
    let leave_dot_git = params.get("leaveDotGit").map(|v| v == "true");
    let fetch_lfs = params.get("fetchLFS").map(|v| v == "true");

    let args = PrefetchArgs {
        url: git_url.clone(),
        rev: rev.to_string(),
        fetch_submodules,
        deep_clone,
        leave_dot_git,
        fetch_lfs,
        branch_name: params.get("branchName").cloned(),
        root_dir: params.get("rootDir").cloned().filter(|v| !v.is_empty()),
        sparse_checkout: sparse_checkout.to_vec(),
    };

    let result = prefetch(&args).map_err(|e| {
        anyhow::anyhow!(
            "Failed to prefetch git repository {} @ {}{}: {e}",
            git_url,
            rev,
            if fetch_submodules.unwrap_or(true) {
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
