# redact: new range
# GitHub commit URL with # follow:branch main comment.
# Tests that fetchpatch revision following works for GitHub commit URLs.
# The # follow:branch main comment instructs the tool to query git ls-remote
# for the latest commit on the main branch and replace the SHA in the
# URL. The hash is also re-computed for the new URL.
# Since the latest SHA on main changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    # follow:branch main
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
  };
}
