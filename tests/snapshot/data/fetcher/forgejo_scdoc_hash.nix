# Pinned fetchFromForgejo with empty hash for ddevault/scdoc on codeberg.org.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://codeberg.org/ddevault/scdoc/archive/1.11.2.tar.gz
{
  src = pkgs.fetchFromForgejo {
    # pin
    domain = "codeberg.org";
    owner = "ddevault";
    repo = "scdoc";
    rev = "1.11.2";
    hash = "";
  };
}
