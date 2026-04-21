# Pinned fetchpatch with hunks option.
# Tests that fetchpatch correctly selects specific hunks when computing
# the hash. With hunks = [1], only the first hunk of each file is
# included in the patch.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch"; hunks = [1]; hash = "sha256-IHot3hq8bCzLj9jlgrdqk+JAOX0zxuVYG80EW58Q5So="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/compare/v0.1.0...v0.2.0.patch";
    hash = "";
    hunks = [ 1 ];
  };
}
