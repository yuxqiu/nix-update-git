# Pinned fetchFromGitea with empty hash for gitea/act on gitea.com.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://gitea.com/gitea/act/archive/v0.261.6.tar.gz
{
  src = pkgs.fetchFromGitea {
    # pin
    domain = "gitea.com";
    owner = "gitea";
    repo = "act";
    rev = "v0.261.6";
    hash = "";
  };
}
