# redact: new range
# SourceHut archive URL with version tag, not pinned.
# Tests that fetchTarball version updates work for SourceHut archive URLs.
# The ref (1.9.3) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://git.sr.ht/~sircmpwn/scdoc/archive/1.9.3.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
