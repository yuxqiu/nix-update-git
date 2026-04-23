# Pinned fetchpatch with executable option.
# Tests that fetchpatch correctly computes the hash when executable = true.
# The executable flag sets the execute bit on the output file, but fetchpatch
# uses outputHashMode = "flat" so the hash depends only on content, not
# permissions. The # pin comment prevents version updates; only the empty
# hash should be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch"; executable = true; hash = "sha256-3T6TF1ol+B8dyzYhwJP2uHCRCGAzDI6kuYaGJAONJi0="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    executable = true;
  };
}
