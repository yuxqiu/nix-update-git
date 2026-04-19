# redact: new range
# GitHub commit URL with # follow:main comment and existing hash.
# Tests that fetchpatch revision following works even when the hash
# is already filled in. The # follow:main comment instructs the tool
# to query git ls-remote for the latest commit on the main branch and
# replace the SHA in the URL. Since the URL changes, the hash must
# also be re-computed to stay in sync with the new patch content.
# Since the latest SHA on main changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    # follow:main
    url = "https://github.com/yuxqiu/nix-update-git/commit/4b60d1ab5349af3a6f9d1b0797aa520b041e7eb3.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}
