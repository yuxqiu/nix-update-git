# Pinned fetchgit with deepClone = true and leaveDotGit = true, empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# both deep clone and leaveDotGit are enabled. This combination fetches
# all tags (--tags) and keeps the .git directory with full history,
# producing a different hash than either flag alone. The # pin comment
# prevents version updates; only the empty hash should be filled.
# Upstream reference: NIX_PREFETCH_GIT_DEEP_CLONE=1 NIX_PREFETCH_GIT_LEAVE_DOT_GIT=1 nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    deepClone = true;
    leaveDotGit = true;
  };
}
