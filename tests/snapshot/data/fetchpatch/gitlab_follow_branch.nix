# redact: new range
# GitLab commit URL with # follow:branch master comment.
# Tests that fetchpatch revision following works for GitLab commit URLs.
# The /-/commit/ path pattern triggers GitLab-specific URL parsing.
# The # follow:branch master comment instructs the tool to query git ls-remote
# for the latest commit on the master branch and replace the SHA in the
# URL. The hash is also re-computed for the new URL.
# Since the latest SHA on master changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    # follow:branch master
    url = "https://gitlab.com/procps-ng/procps/-/commit/0000000000000000000000000000000000000000.patch";
    hash = "";
  };
}
