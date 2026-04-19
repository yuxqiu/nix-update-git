# Pinned fetchpatch with empty hash on a multi-file patch.
# Tests that the patch normalization correctly handles patches that modify
# multiple files (sorting sections alphabetically by path, removing diff --git
# headers, index lines, etc.), producing the same flat SHA-256 hash as nixpkgs'
# fetchpatch would compute for the same URL.
# Uses a commit that touches two files (Cargo.lock and Cargo.toml).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch"; hash = "sha256-VyL5gqrmdW4p5ZUDiUIyQy+i076bwOwrQsm016A5XvQ="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch";
    hash = "";
  };
}
