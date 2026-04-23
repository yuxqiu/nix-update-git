# redact: new range
# Pinned fetchpatch with netrcImpureEnvVars.
# Tests that fetchpatch skips hash computation when netrcImpureEnvVars
# is set, because these environment variables can provide auth credentials
# that change what gets downloaded.
# The # pin comment prevents version updates, and netrcImpureEnvVars
# prevents hash prefetching, so no updates should be produced.
{
  patch = fetchpatch {
    # pin
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "";
    netrcImpureEnvVars = [ "NETRC" ];
  };
}
