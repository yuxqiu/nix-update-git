# Pinned fetchgit with fetchSubmodules = true and leaveDotGit = true on a
# repo that has submodules. Tests that the Rust nix-prefetch-git computes
# the correct hash when both submodule fetching and .git retention are
# enabled simultaneously. This combination exercises the deep submodule
# update path (without --depth=1) because leaveDotGit triggers the
# non-shallow submodule strategy. The # pin comment prevents version
# updates; only the empty hash should be filled.
# Upstream reference: NIX_PREFETCH_GIT_LEAVE_DOT_GIT=1 nix-prefetch-git --url https://github.com/expipiplus1/has-submodule --rev a-tag --fetch-submodules
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/has-submodule";
    rev = "a-tag"; # pin
    hash = "";
    fetchSubmodules = true;
    leaveDotGit = true;
  };
}
