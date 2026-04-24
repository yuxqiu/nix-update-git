# redact: new range
# GitHub archive URL with .zip extension, not pinned.
# Tests that fetchTarball handles .zip archive URLs (not just .tar.gz).
# The ref (v0.1.0) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.zip";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
