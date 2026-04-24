# fetchTarball with a non-version URL (commit SHA) should not produce updates.
# The URL contains a commit SHA rather than a version tag, so the tool
# cannot determine a newer version to update to.
{
  src = fetchTarball {
    url = "https://github.com/yuxqiu/nix-update-git/archive/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.tar.gz";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  };
}
