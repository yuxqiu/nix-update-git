# Tests that fetchFromGitHub computes the correct hash for a pinned tag.
# The # pin comment prevents version updates; only the empty hash should be filled.
{
  src = pkgs.fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.0"; # pin
    hash = "";
  };
}
