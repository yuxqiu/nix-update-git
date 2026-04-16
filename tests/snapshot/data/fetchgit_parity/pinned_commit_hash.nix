# Pinned fetchgit with commit hash ref and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# the rev is a full commit SHA (not a symbolic tag). Commit hashes are
# resolvable locally after a shallow fetch, unlike tag names which only
# exist in FETCH_HEAD. The # pin comment prevents version updates; only
# the empty hash should be filled.
# Upstream reference: nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev 22f12ea408b1ff7a0a8104268d975e541c6a7df2
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "22f12ea408b1ff7a0a8104268d975e541c6a7df2"; # pin
    hash = "";
  };
}
