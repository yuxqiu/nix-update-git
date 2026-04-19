# Pinned fetchpatch with empty hash on a GitLab commit.
# Tests that the patch normalization produces the same flat SHA-256
# hash as nixpkgs' fetchpatch would compute for a GitLab commit URL.
# The /-/commit/ path pattern is specific to GitLab.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Verify with: nix-build -E 'let pkgs = import <nixpkgs> {}; in pkgs.fetchpatch { url = "https://gitlab.com/procps-ng/procps/-/commit/4ddcef2fd843170c8e2d59a83042978f41037a2b.patch"; hash = "sha256-PC8yHwlRETyc32TfRx2b05tqnKW8D4ehMrVjWmbHYz0="; }'
{
  patch = fetchpatch {
    # pin
    url = "https://gitlab.com/procps-ng/procps/-/commit/4ddcef2fd843170c8e2d59a83042978f41037a2b.patch";
    hash = "";
  };
}
