# Pinned fetchTarball with empty hash (SRI format) on GitHub archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    hash = "";
  };
}
