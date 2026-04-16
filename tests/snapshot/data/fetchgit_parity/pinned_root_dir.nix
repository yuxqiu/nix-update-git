# Pinned fetchgit with rootDir = "src" and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# only a subdirectory of the repository is used as the top-level output.
# The rootDir option causes only the contents of the specified directory
# to be NAR-hashed, producing a different hash than the full checkout.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1 --root-dir src
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    rootDir = "src";
  };
}
