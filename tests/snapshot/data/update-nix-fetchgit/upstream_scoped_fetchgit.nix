# redact: new
#
# Adapted from upstream test: test_scoped.in.nix
# Tests that scoped names like pkgs.fetchgit are recognized.
{
  src = pkgs.fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1";
    hash = "";
  };
}
