# Tests that fetchFromGitHub computes the correct nix-base32 hash when
# using sha256 instead of the SRI hash field.
# The # pin comment prevents version updates; only the empty sha256 should be filled.
{
  src = pkgs.fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.0"; # pin
    sha256 = "";
  };
}
