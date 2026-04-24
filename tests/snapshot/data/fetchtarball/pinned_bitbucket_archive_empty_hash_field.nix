# Pinned fetchTarball with empty hash (SRI format) on Bitbucket archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://bitbucket.org/atlassian/atlassian-rest/get/atlassian-rest-parent-8.1.3.tar.gz";
    hash = "";
  };
}
