# Pinned fetchFromGitiles with empty hash for Android Verified Boot (avb).
# The # pin comment prevents version updates; only the empty hash should
# be filled using the tarball URL:
#   https://android.googlesource.com/platform/external/avb/+archive/android-12.0.0_r1.tar.gz
{
  src = pkgs.fetchFromGitiles {
    # pin
    url = "https://android.googlesource.com/platform/external/avb";
    rev = "android-12.0.0_r1";
    hash = "";
  };
}
