# Pinned fetchpatch with combined stripLen and relative options.
# Tests that fetchpatch correctly applies both stripLen and relative
# when computing the hash. With relative = "src" and stripLen = 1,
# the effective strip count is 1 + 1 + 1 = 3 (one for the a/ prefix,
# one for the relative path segment, and the user-specified stripLen).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch"; relative = "src"; stripLen = 1; hash = "sha256-/oUcwh0nByTTNArfQ4qmCSuDe9Orya7BZULcaayHhFc="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch";
    hash = "";
    relative = "src";
    stripLen = 1;
  };
}
