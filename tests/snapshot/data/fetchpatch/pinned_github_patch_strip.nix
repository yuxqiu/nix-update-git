# Pinned fetchpatch with stripLen = 1 and empty hash.
# Tests that fetchpatch correctly applies path stripping when computing
# the hash. With stripLen = 1, one leading path component is stripped
# from all file paths in the patch before hashing.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/57016334304d2a1494b2ae3f0ee39b0027ed5dc3.patch";
    hash = "";
    stripLen = 1;
  };
}
