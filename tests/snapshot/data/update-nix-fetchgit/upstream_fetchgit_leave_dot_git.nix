# redact: new
#
# Adapted from upstream test: test_dotgit.in.nix
# Tests fetchgit with leaveDotGit flag.
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1";
    sha256 = "";
    leaveDotGit = true;
  };
}
