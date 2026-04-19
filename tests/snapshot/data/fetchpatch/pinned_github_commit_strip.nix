# Pinned fetchpatch with stripLen = 1 and empty hash.
# Tests that the patch normalization correctly applies path stripping
# when computing the hash. With stripLen = 1, one leading path component
# is stripped from all file paths before hashing, matching nixpkgs'
# fetchpatch behavior with stripLen.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/57016334304d2a1494b2ae3f0ee39b0027ed5dc3.patch"; stripLen = 1; hash = "sha256-uZDiDTW46guXczBUpf9aIN2x3I9OW6YWgQ1l4R6q4f0="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/57016334304d2a1494b2ae3f0ee39b0027ed5dc3.patch";
    hash = "";
    stripLen = 1;
  };
}
