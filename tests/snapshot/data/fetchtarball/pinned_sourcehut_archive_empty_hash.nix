# Pinned fetchTarball with empty hash on SourceHut archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://git.sr.ht/~sircmpwn/scdoc/archive/1.9.3.tar.gz";
    sha256 = "";
  };
}
