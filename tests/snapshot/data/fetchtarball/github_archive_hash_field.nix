# redact: new range
# GitHub archive URL using hash field instead of sha256, not pinned.
# Tests that fetchTarball handles the hash field (SRI format) instead of sha256.
# The ref (v0.1.0) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  src = fetchTarball {
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
