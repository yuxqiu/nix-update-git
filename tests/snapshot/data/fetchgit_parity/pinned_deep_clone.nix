# Pinned fetchgit with deepClone = true and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# performing a deep clone (full history). Deep clones without leaveDotGit
# produce the same hash as shallow clones because .git is stripped.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Upstream reference: NIX_PREFETCH_GIT_DEEP_CLONE=1 nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    deepClone = true;
  };
}
