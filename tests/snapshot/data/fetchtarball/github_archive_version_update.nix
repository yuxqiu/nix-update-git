# redact: new range
# GitHub archive URL with version tag, not pinned.
# Tests that fetchTarball version updates work for GitHub archive URLs.
# The ref (v0.1.0) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# The hash is also re-computed for the new URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
