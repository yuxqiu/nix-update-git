# Adapted from upstream test: test_networked/test_github_submodules.in.nix
# Tests fetchFromGitHub with fetchSubmodules = true.
{
  regular = pkgs.fetchFromGitHub {
    owner = "expipiplus1";
    repo = "has-submodule";
    rev = "a-tag";
    hash = "";
    fetchSubmodules = true;
  };
}
