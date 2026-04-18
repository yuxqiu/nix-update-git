# Pinned fetchpatch with empty hash using pkgs.fetchpatch prefix.
# Tests that fetchpatch is recognized even when called as pkgs.fetchpatch
# (with the pkgs. prefix), matching how it's commonly used in nixpkgs.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
{
  patch = pkgs.fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
  };
}
