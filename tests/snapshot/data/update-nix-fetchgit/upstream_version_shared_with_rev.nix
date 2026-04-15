# redact: new
#
# Adapted from upstream issue #34: mkDerivation pattern where version is shared
# with the fetcher rev. In this pattern, rev = version and the fetcher rule
# should handle updating the version string.
{
  foo = mkDerivation {
    version = "v0.2.1";
    src = fetchFromGitHub {
      owner = "expipiplus1";
      repo = "update-nix-fetchgit";
      rev = "v0.2.1";
      hash = "";
    };
  };
}
