# Pinned fetchpatch with empty sha256 field (nix-base32 format).
# Tests that the patch normalization produces the same nix-base32 sha256
# hash as nixpkgs' fetchpatch would compute for the same URL.
# The # pin comment prevents version updates; only the empty sha256 should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch"; sha256 = "1gs39l5fdcd1ilwzzmgkl11y68lzlwblgq9lxh3asqfzw658mw0k"; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    sha256 = "";
  };
}
