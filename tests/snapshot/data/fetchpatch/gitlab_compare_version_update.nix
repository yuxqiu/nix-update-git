# redact: new range
# GitLab compare URL with version-like head ref, not pinned.
# Tests that fetchpatch version updates work for GitLab compare URLs.
# The /-/compare/ path pattern triggers GitLab-specific URL parsing.
# The head ref (v4.0.3) looks like a version, so the tool queries
# git ls-remote for the latest matching tag and replaces it in the URL.
# The hash is also re-computed for the new URL.
# Since the latest version tag changes over time, `new` and `range`
# are redacted.
{
  patch = fetchpatch {
    url = "https://gitlab.com/procps-ng/procps/-/compare/v4.0.2...v4.0.3.patch";
    hash = "";
  };
}
