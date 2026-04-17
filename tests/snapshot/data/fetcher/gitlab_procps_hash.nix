# Pinned fetchFromGitLab with empty hash for procps-ng/procps on gitlab.com.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://gitlab.com/procps-ng/procps/-/archive/v4.0.3/procps-v4.0.3.tar.gz
{
  src = pkgs.fetchFromGitLab {
    # pin
    owner = "procps-ng";
    repo = "procps";
    rev = "v4.0.3";
    hash = "";
  };
}
