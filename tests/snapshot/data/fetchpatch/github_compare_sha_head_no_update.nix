# GitHub compare URL with SHA head ref, not pinned.
# Tests that fetchpatch does not attempt version updates when the
# head of a compare range is a commit SHA rather than a version-like ref.
# The head ref (4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3) is a 40-char
# hex SHA, which is not a version, so no version update should occur.
# With a non-empty hash, no hash prefetch is needed either,
# so the result should be empty (no updates).
{
  patch = fetchpatch {
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
