# redact: new range
# Pinned fetchpatch with curlOptsList.
# Tests that fetchpatch skips hash computation when curlOptsList is set,
# because curlOptsList injects arbitrary curl flags that can change what
# gets downloaded. The hash should remain empty (not computed).
# The # pin comment prevents version updates, and curlOptsList prevents
# hash prefetching, so no updates should be produced for this file.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    curlOptsList = [
      "-H"
      "Accept: application/diff"
    ];
  };
}
