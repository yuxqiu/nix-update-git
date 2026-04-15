# Adapted from upstream test: test_networked/test_readme_examples.in.nix
# Tests fetchFromGitHub with pinned rev and empty hash.
{
  upfind = pkgs.fetchFromGitHub {
    owner = "expipiplus1";
    repo = "upfind";
    rev = "cb451254f5b112f839aa36e5b6fd83b60cf9b9ae"; # pin
    hash = "";
  };
}
