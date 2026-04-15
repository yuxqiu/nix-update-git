# redact: new
#
# Adapted from upstream test: test_scoped.in.nix
# Tests deeply scoped names like foo.bar.pkgs.fetchgit.
{
  src = foo.bar.pkgs.fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1";
    hash = "";
  };
}
