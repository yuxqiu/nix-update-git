# Pinned fetchpatch with empty sha256 field.
# Tests that fetchpatch fills in the sha256 hash (Nix32 format)
# when sha256 is used instead of hash.
# The # pin comment prevents version updates; only the empty sha256 should
# be filled.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    sha256 = "";
  };
}
