# Pinned fetchgit with sha256 field (nix-base32 format) and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct nix-base32
# hash when the fetcher uses sha256 instead of the SRI hash field.
# The # pin comment prevents version updates; only the empty sha256
# should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    sha256 = "";
  };
}
