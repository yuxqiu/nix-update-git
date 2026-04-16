# Pinned fetchgit with fetchSubmodules = true on a repo that has submodules.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# fetching submodules recursively. The has-submodule repo contains a
# submodule pointing at update-nix-fetchgit. The # pin comment prevents
# version updates; only the empty hash should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/has-submodule --rev a-tag --fetch-submodules
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/has-submodule";
    rev = "a-tag"; # pin
    hash = "";
    fetchSubmodules = true;
  };
}
