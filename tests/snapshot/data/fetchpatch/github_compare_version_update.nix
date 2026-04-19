# redact: new range
# GitHub compare URL with version-like head ref, not pinned.
# Tests that fetchpatch version updates work for GitHub compare URLs.
# The head ref (v0.2.0) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# The hash is also re-computed for the new URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch";
    hash = "";
  };
}
