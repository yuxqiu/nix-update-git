# fetchpatch with non-empty hash.
# Tests that fetchpatch does not produce any updates when the hash
# is already filled in. No # pin comment is needed because the
# non-empty hash prevents hash prefetching, and fetchpatch has no
# version-based update logic (it only fills empty hashes).
{
  patch = fetchpatch {
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
