# Pinned fetchpatch with empty hash field.
# Tests that the patch normalization (remove diff --git headers, index lines,
# hunk context text, sort sections by path) produces the same flat SHA-256
# hash as nixpkgs' fetchpatch would compute for the same URL.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch"; hash = "sha256-E/CKiuHfYa0G7DThRxennyLjQ6Dz1f85jaGx5gpNQ78="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
  };
}
