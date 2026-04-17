# Pinned fetchFromSourcehut with empty hash for ~sircmpwn/scdoc on git.sr.ht.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://git.sr.ht/~sircmpwn/scdoc/archive/1.9.3.tar.gz
{
  src = pkgs.fetchFromSourcehut {
    # pin
    owner = "~sircmpwn";
    repo = "scdoc";
    rev = "1.9.3";
    hash = "";
  };
}
