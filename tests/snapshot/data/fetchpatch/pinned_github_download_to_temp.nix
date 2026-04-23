# redact: new range
# Pinned fetchpatch with downloadToTemp option.
# Tests that fetchpatch still computes the hash when downloadToTemp is true.
# downloadToTemp only changes where the downloaded file is placed in the build,
# not the content itself, so the flat hash computation is unaffected.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    downloadToTemp = true;
  };
}
