# redact: new range
# Gitea archive URL with version tag, not pinned.
# Tests that fetchTarball version updates work for Gitea archive URLs.
# The ref (v0.261.6) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://gitea.com/gitea/act/archive/v0.261.6.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
