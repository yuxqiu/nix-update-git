# redact: new range
# Pinned fetchpatch with non-sha256 outputHashAlgo.
# Tests that fetchpatch skips hash computation when outputHashAlgo is not
# sha256, because the tool only computes SHA-256 hashes and cannot produce
# hashes for other algorithms.
# The # pin comment prevents version updates, and the mismatched algo
# prevents hash prefetching, so no updates should be produced.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    outputHash = "";
    outputHashAlgo = "sha512";
  };
}
