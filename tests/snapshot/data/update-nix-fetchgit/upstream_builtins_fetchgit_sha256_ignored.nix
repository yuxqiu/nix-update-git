# redact: new
#
# Adapted from upstream test: test_builtins_fetchgit_update_ignores_sha256.in.nix
# Tests that builtins.fetchGit ignores sha256 even when present.
{
  src = builtins.fetchGit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    ref = "v0.2.1";
    sha256 = "IGNORED";
  };
}
