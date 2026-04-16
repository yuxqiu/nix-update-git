# Pinned fetchgit with leaveDotGit = true and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# the .git directory is kept in the checkout (made deterministic via
# repacking, garbage collection, and stripping non-reproducible metadata).
# This exercises the git-clone path (not tarball) because leaveDotGit
# forces HashStrategy::Git. The # pin comment prevents version updates;
# only the empty hash should be filled.
# Upstream reference: NIX_PREFETCH_GIT_LEAVE_DOT_GIT=1 nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    leaveDotGit = true;
  };
}
