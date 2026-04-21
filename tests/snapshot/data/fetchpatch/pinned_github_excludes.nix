# Pinned fetchpatch with excludes option.
# Tests that fetchpatch correctly applies the excludes filter when
# computing the hash. With excludes = ["Cargo.lock"], the Cargo.lock
# file changes are excluded from the patch.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch"; excludes = ["Cargo.lock"]; hash = "sha256-WxdZtUQVjttjUndYn/AtGGREBbMQtsEPpCbqUnfgLfE="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch";
    hash = "";
    excludes = [ "Cargo.lock" ];
  };
}
