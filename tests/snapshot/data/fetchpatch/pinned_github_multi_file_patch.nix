# Pinned fetchpatch with empty hash on a multi-file patch.
# Tests that fetchpatch correctly normalizes patches that modify multiple
# files (sorting sections alphabetically by path, removing diff --git
# headers, index lines, etc.). Uses a commit that touches two files
# (Cargo.lock and Cargo.toml).
# The # pin comment prevents version updates; only the empty hash should
# be filled.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/7e2aa250605112cdedbc76f41cda0eb84184788a.patch";
    hash = "";
  };
}
