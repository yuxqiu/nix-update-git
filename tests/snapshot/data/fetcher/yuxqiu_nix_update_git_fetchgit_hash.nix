# Tests that fetchgit computes the correct hash for a pinned tag.
# The # pin comment prevents version updates; only the empty hash should be filled.
{
  src = pkgs.fetchgit {
    url = "https://github.com/yuxqiu/nix-update-git";
    rev = "v0.1.0"; # pin
    hash = "";
  };
}
