# Pinned fetchFromBitbucket with empty hash for atlassian/atlassian-rest.
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://bitbucket.org/atlassian/atlassian-rest/get/atlassian-rest-parent-8.1.3.tar.gz
{
  src = pkgs.fetchFromBitbucket {
    # pin
    owner = "atlassian";
    repo = "atlassian-rest";
    rev = "atlassian-rest-parent-8.1.3";
    hash = "";
  };
}
