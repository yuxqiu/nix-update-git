# Pinned fetchpatch with extraPrefix option.
# Tests that fetchpatch correctly applies the extraPrefix when
# computing the hash. With extraPrefix = "pkg", the prefix a/pkg/
# is added to old paths and b/pkg/ is added to new paths.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch"; extraPrefix = "pkg"; hash = "sha256-Nw/dnwdbi6uajwS8fNr/BeheKvF62Iu0Nsj9msR4McI="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    extraPrefix = "pkg";
  };
}
