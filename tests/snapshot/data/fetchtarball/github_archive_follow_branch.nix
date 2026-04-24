# redact: new range
# GitHub archive URL with # follow:branch main comment.
# Tests that fetchTarball branch following works for GitHub archive URLs.
# The # follow:branch main comment instructs the tool to query git ls-remote
# for the latest commit on the main branch and replace the ref in the URL.
# The hash is also re-computed for the new URL.
# Since the latest SHA on main changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    # follow:branch main
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
