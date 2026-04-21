# Pinned fetchpatch with relative option.
# Tests that fetchpatch correctly applies the relative filter when
# computing the hash. With relative = "src", only files under src/
# are included, and the strip count is adjusted accordingly.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch"; relative = "src"; hash = "sha256-3go+ewnwO1pcxbNcCymxaAU67n6OsLKF7lDgG4nPa00="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch";
    hash = "";
    relative = "src";
  };
}
