# Pinned fetchpatch with includes option.
# Tests that fetchpatch correctly applies the includes filter when
# computing the hash. With includes = ["Cargo.toml"], only the
# Cargo.toml file changes are included in the patch.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch"; includes = ["Cargo.toml"]; hash = "sha256-WxdZtUQVjttjUndYn/AtGGREBbMQtsEPpCbqUnfgLfE="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch";
    hash = "";
    includes = [ "Cargo.toml" ];
  };
}
