# Pinned fetchTarball with empty hash (SRI format) on Gitea archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://gitea.com/gitea/act/archive/v0.261.6.tar.gz";
    hash = "";
  };
}
