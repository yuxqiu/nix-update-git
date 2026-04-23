# Pinned fetchpatch with empty outputHash.
# Tests that fetchpatch fills in empty outputHash with the computed
# SHA-256 SRI hash. outputHash with outputHashAlgo defaults to sha256
# in Nix, so an empty outputHash can be filled with our computed hash.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    outputHash = "";
    outputHashAlgo = "sha256";
  };
}
