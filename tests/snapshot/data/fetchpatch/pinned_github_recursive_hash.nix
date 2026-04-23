# redact: new range
# Pinned fetchpatch with recursiveHash option.
# Tests that fetchpatch skips hash computation when recursiveHash is true,
# because recursive hashing uses a different hash mode (outputHashMode = "recursive")
# than what fetchpatch normally uses (flat hashing).
# The # pin comment prevents version updates, and recursiveHash prevents
# hash prefetching, so no updates should be produced for this file.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    recursiveHash = true;
  };
}
