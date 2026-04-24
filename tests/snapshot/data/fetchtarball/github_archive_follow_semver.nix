# redact: new range
# GitHub archive URL with # follow:semver ^0.1 comment.
# Tests that fetchTarball semver following works for GitHub archive URLs.
# The # follow:semver ^0.1 comment instructs the tool to find the latest
# semver-compatible tag and replace the ref in the URL.
# Since the matching tag changes over time, `new` and `range` are redacted.
{
  src = fetchTarball {
    # follow:semver ^0.1
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
