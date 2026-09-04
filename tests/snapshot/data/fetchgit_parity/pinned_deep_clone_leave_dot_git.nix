# redact: new
#
# Pinned fetchgit with deepClone = true and leaveDotGit = true, empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# both deep clone and leaveDotGit are enabled, the same behavior
# pinned_deep_clone.nix gets implicitly, since `leaveDotGit` defaults to
# `deepClone`'s value when unset. The clone is set up via `git init` +
# `remote add` (not `git clone --single-branch`), so a plain
# `git fetch --tags origin` pulls every branch (the remote's default
# refspec) plus every tag currently on the remote, not just the history
# reachable from the pinned rev. That makes the resulting hash a function
# of the upstream repo's *entire current state* (all branches and tags),
# not just the pinned commit, so it legitimately drifts as upstream keeps
# committing/tagging, hence the redact above. Verified empirically:
# recomputing this hash independently reproduces whatever CI currently
# gets, confirming it's deterministic-for-a-given-remote-state, not flaky.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Upstream reference: NIX_PREFETCH_GIT_DEEP_CLONE=1 NIX_PREFETCH_GIT_LEAVE_DOT_GIT=1 nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    deepClone = true;
    leaveDotGit = true;
  };
}
