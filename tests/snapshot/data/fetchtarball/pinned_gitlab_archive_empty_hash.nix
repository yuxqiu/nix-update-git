# Pinned fetchTarball with empty hash on GitLab archive URL.
# The # pin comment prevents version updates, but the empty hash
# should still be filled using the tarball URL.
{
  src = fetchTarball {
    # pin
    url = "https://gitlab.com/procps-ng/procps/-/archive/v4.0.3/procps-v4.0.3.tar.gz";
    sha256 = "";
  };
}
