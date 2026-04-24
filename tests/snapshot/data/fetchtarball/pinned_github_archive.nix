# fetchTarball with a pinned call should not produce updates.
# The # pin comment prevents version detection; only empty hash filling
# would apply, but the hash here is non-empty.
{
  src = fetchTarball {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/archive/v0.1.0.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
