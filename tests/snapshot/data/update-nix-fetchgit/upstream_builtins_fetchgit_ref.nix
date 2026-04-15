# redact: new
#
# Adapted from upstream test: test_builtins_fetchgit.in.nix
# Tests builtins.fetchGit with rev update.
{
  src = builtins.fetchGit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    ref = "v0.2.1";
  };
}
