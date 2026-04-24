# redact: new range
# GitHub archive URL with refs/tags/ prefix in the path, not pinned.
# Tests that fetchTarball handles the /archive/refs/tags/ URL pattern.
# The ref (v0.1.0) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://github.com/yuxqiu/nix-update-git/archive/refs/tags/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
