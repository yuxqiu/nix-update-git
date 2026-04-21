# Pinned fetchpatch with revert option.
# Tests that fetchpatch correctly reverses the patch when computing
# the hash. With revert = true, the patch is reversed (insert/remove
# lines are swapped, old/new paths are swapped).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch"; revert = true; hash = "sha256-3T6TF1ol+B8dyzYhwJP2uHCRCGAzDI6kuYaGJAONJi0="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    revert = true;
  };
}
