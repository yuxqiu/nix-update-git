# Pinned fetchgit with deepClone = true, fetchSubmodules = true, and
# leaveDotGit = true on a repo that has submodules. Tests that the Rust
# nix-prefetch-git computes the correct hash when all three git-specific
# flags are enabled simultaneously. This combination exercises the full
# deep-clone path: --tags fetch, recursive submodule checkout (without
# --depth=1), and deterministic .git retention with full history. This
# produces a distinct hash from any subset of these flags. The # pin
# comment prevents version updates; only the empty hash should be filled.
# Upstream reference: NIX_PREFETCH_GIT_DEEP_CLONE=1 NIX_PREFETCH_GIT_LEAVE_DOT_GIT=1 nix-prefetch-git --url https://github.com/expipiplus1/has-submodule --rev a-tag --fetch-submodules
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/has-submodule";
    rev = "a-tag"; # pin
    hash = "";
    deepClone = true;
    fetchSubmodules = true;
    leaveDotGit = true;
  };
}
