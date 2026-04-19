# GitHub commit URL with # pin comment and existing hash.
# Tests that pinned fetchpatch does not attempt version updates or
# revision following. The # pin comment prevents any version-based
# updates. With a non-empty hash, no hash prefetch is needed either,
# so the result should be empty (no updates).
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
