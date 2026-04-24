# redact: new range
# GitHub archive URL with # follow:regex v0\.1\.\d+ comment.
# Tests that fetchTarball regex following works for GitHub archive URLs.
# The regex is anchored by the tool (^(?:regex)$), so v0\.1\.\d+ matches
# tags like v0.1.0 and v0.1.1. The latest matching tag replaces the ref.
# Since the matching tag changes over time, `new` and `range` are redacted.
{
  src = fetchTarball {
    # follow:regex v0\.1\.\d+
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
