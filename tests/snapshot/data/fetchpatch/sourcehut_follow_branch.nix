# redact: new range
# SourceHut commit URL with # follow:master comment.
# Tests that fetchpatch revision following works for SourceHut commit URLs.
# The git.sr.ht domain and ~owner prefix in the path trigger SourceHut-specific
# URL parsing. The # follow:master comment instructs the tool to query
# git ls-remote for the latest commit on the master branch and replace the
# SHA in the URL. The hash is also re-computed for the new URL.
# Since the latest SHA on master changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    # follow:master
    url = "https://git.sr.ht/~sircmpwn/scdoc/commit/0000000000000000000000000000000000000000.patch";
    hash = "";
  };
}
