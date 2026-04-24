# redact: new range
# GitLab archive URL with version tag, not pinned.
# Tests that fetchTarball version updates work for GitLab archive URLs.
# The /-/archive/ path pattern triggers GitLab-specific URL parsing.
# The ref (v4.0.3) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://gitlab.com/procps-ng/procps/-/archive/v4.0.3/procps-v4.0.3.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
