# redact: new range
# Bitbucket archive URL with version tag, not pinned.
# Tests that fetchTarball version updates work for Bitbucket archive URLs.
# The ref (atlassian-rest-parent-8.1.3) looks like a version, so the tool
# queries git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://bitbucket.org/atlassian/atlassian-rest/get/atlassian-rest-parent-8.1.3.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
