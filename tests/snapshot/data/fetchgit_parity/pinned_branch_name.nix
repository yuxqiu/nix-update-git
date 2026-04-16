# Pinned fetchgit with branchName = "custom-branch" and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# a custom branch name is used for the local checkout. The branchName
# option controls which branch name is used for `git checkout -b`, but
# does not affect the NAR hash since only file content matters. The
# default branch name is "fetchgit". The # pin comment prevents version
# updates; only the empty hash should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1 --branch-name custom-branch
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    branchName = "custom-branch";
  };
}
