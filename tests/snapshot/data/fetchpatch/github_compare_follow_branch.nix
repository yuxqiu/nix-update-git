# redact: new range
# GitHub compare URL with # follow:main comment.
# Tests that fetchpatch revision following works for GitHub compare URLs.
# When # follow:main is present on a compare URL, the tool queries
# git ls-remote for the latest commit on the main branch and replaces
# the head ref in the compare range. The hash is also re-computed for
# the new URL.
# Since the latest SHA on main changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    # follow:main
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
  };
}
