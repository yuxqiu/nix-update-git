# Pinned fetchgit with tag ref and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash for
# a shallow fetch of a tag reference. The # pin comment prevents
# version updates; only the empty hash should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
  };
}
