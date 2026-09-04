# redact: new
#
# Pinned fetchgit with deepClone = true and empty hash.
# Tests that the Rust nix-prefetch-git computes the correct hash when
# performing a deep clone (full history). `leaveDotGit` isn't set here, so
# it defaults to `deepClone`'s value (true, matching nixpkgs' own
# `leaveDotGit = deepClone || fetchTags` default): .git is kept, and the
# fetch pulls `--tags`, so this is equivalent to
# pinned_deep_clone_leave_dot_git.nix, not to a shallow clone. The clone is
# set up via `git init` + `remote add` (not `git clone --single-branch`),
# so a plain `git fetch --tags origin` pulls every branch (the remote's
# default refspec) plus every tag currently on the remote, not just the
# history reachable from the pinned rev. That makes the resulting hash a
# function of the upstream repo's *entire current state* (all branches and
# tags), not just the pinned commit, so it legitimately drifts as upstream
# keeps committing/tagging, hence the redact above. Verified empirically:
# recomputing this hash independently reproduces whatever CI currently
# gets, confirming it's deterministic-for-a-given-remote-state, not flaky.
# The # pin comment prevents version updates; only the empty hash should
# be filled.
# Upstream reference: NIX_PREFETCH_GIT_DEEP_CLONE=1 nix-prefetch-git --url https://github.com/expipiplus1/update-nix-fetchgit --rev v0.2.1
{
  src = fetchgit {
    url = "https://github.com/expipiplus1/update-nix-fetchgit";
    rev = "v0.2.1"; # pin
    hash = "";
    deepClone = true;
  };
}
