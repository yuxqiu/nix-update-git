# redact: new range
# Pinned fetchpatch with sha1 attribute.
# Tests that fetchpatch skips hash computation when sha1 is present with a
# non-empty value, because sha1 specifies a different hash algorithm than
# SHA-256. The tool only computes SHA-256 hashes and cannot fill in sha1.
# The # pin comment prevents version updates, and the existing sha1 value
# prevents hash prefetching, so no updates should be produced.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    sha1 = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
  };
}
