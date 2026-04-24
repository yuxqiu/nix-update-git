# Pinned fetchTarball with empty hash on Codeberg archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://codeberg.org/ddevault/scdoc/archive/1.11.2.tar.gz";
    sha256 = "";
  };
}
