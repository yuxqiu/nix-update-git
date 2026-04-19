# Pinned fetchpatch with empty hash on a GitHub compare URL.
# Tests that the patch normalization produces the same flat SHA-256
# hash as nixpkgs' fetchpatch for compare/merge-request URLs that
# generate multi-commit patches (with email headers, preamble content,
# and sections that modify/create/delete files across multiple commits).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch"; hash = "sha256-G+lNFpCD+840NFLX6e5aMkFYb2DDCHcmEZbeosR3tGQ="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch";
    hash = "";
  };
}
