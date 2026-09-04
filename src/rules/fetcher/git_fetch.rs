use anyhow::Result;
use nix_prefetch_git::{NarHash, PrefetchArgs, prefetch};

use crate::parser::ParsedAttrs;

use super::kind::FetcherKind;

/// # Errors
///
/// Returns an error if `kind` has no resolvable git URL, or if cloning the
/// repository and computing its NAR hash fails.
pub fn compute_hash(
    kind: &FetcherKind,
    parsed: &ParsedAttrs,
    rev: &str,
    sparse_checkout: &[String],
) -> Result<NarHash> {
    let Some(git_url) = kind.git_url(parsed) else {
        anyhow::bail!("No git URL available")
    };

    let submodules_key = if kind == &FetcherKind::BuiltinsFetchGit {
        "submodules"
    } else {
        "fetchSubmodules"
    };

    let fetch_submodules = parsed.bools.get(submodules_key).copied();
    let deep_clone = parsed.bools.get("deepClone").copied();
    let leave_dot_git = parsed.bools.get("leaveDotGit").copied();
    let fetch_lfs = parsed.bools.get("fetchLFS").copied();

    let args = PrefetchArgs {
        url: git_url.clone(),
        rev: rev.to_string(),
        fetch_submodules,
        deep_clone,
        leave_dot_git,
        fetch_lfs,
        branch_name: parsed.strings.get("branchName").cloned(),
        root_dir: parsed
            .strings
            .get("rootDir")
            .cloned()
            .filter(|v| !v.is_empty()),
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
