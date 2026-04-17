# Pinned fetchFromRepoOrCz with empty hash for girocco on repo.or.cz.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://repo.or.cz/girocco.git/snapshot/girocco-1.0.tar.gz
{
  src = pkgs.fetchFromRepoOrCz {
    # pin
    repo = "girocco";
    rev = "girocco-1.0";
    hash = "";
  };
}
