# Pinned fetchpatch with empty hash.
# Tests that fetchpatch is recognized and its hash is filled
# using flat SHA-256 hashing after normalizing the patch content
# (removing diff --git headers, index lines, hunk context, etc.).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Upstream reference: nixpkgs fetchpatch normalization
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
  };
}
