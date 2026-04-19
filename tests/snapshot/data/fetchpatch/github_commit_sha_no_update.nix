# GitHub commit URL without # pin, non-version SHA head.
# Tests that fetchpatch does not attempt version updates when the
# URL contains a commit SHA (which is not a version-like ref).
# Without a # pin comment, version updates would only be attempted
# if the ref in the URL looks like a version (e.g. v2.0.0). A 40-char
# hex SHA is not a version, so no version update should occur.
# With a non-empty hash, no hash prefetch is needed either,
# so the result should be empty (no updates).
{
  patch = fetchpatch {
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
