# Plan: Eliminate `nix-prefetch-git` Dependency (Strategy A)

The goal is to compute NAR hashes entirely in Rust, without requiring `nix-prefetch-git`
or any other Nix tool on the host system. This is broken into small, independently
shippable steps ordered by impact and complexity.

## Background

Nix computes content hashes by serializing a filesystem tree as a NAR (Nix Archive)
and then SHA-256'ing the resulting byte stream. Different fetchers materialize
different directory trees from the same git commit, producing different NAR hashes:

| Fetcher | Materialization | Hash source |
|---|---|---|
| `fetchFromGitHub` (default) | Downloads tarball from GitHub archive API, unpacks | Tarball tree |
| `fetchFromGitLab` (default) | Downloads tarball from GitLab archive API, unpacks | Tarball tree |
| `fetchgit` | `git clone` → checkout → strip `.git` | Working tree |
| `builtins.fetchGit` | Reads git tree objects via libgit2 | Git tree objects |

Strategy B (already implemented) covers the tarball path. Strategy A completes the
remaining fetcher kinds.

---

## Step 1: Tarball hashing for Gitea/Forgejo

**Status**: Not started

Add tarball archive URL support for Gitea/Forgejo instances. These platforms
provide deterministic tarball archives at predictable URLs, similar to GitHub
and GitLab.

**Changes**:
- Add `FetcherKind::FetchFromGitea` and `FetcherKind::FetchFromForgejo` to
  the `uses_tarball()` check in `src/rules/fetcher.rs`
- Implement archive URL construction in `compute_tarball_hash()`:
  - Gitea: `https://{domain}/api/v1/repos/{owner}/{repo}/archive/{rev}.tar.gz`
  - Forgejo: same URL pattern as Gitea (API-compatible)
- Add unit tests for URL construction
- Add integration test gated behind `network-tests` feature

**Verification**: Run `nix-update-git --update` on a Gitea/Forgejo-sourced nix
file and confirm the computed hash matches what Nix produces.

---

## Step 2: Tarball hashing for Sourcehut, Bitbucket, Savannah, repo.or.cz

**Status**: Not started

Extend tarball hashing to remaining forge-style fetchers that provide archive
APIs.

**Changes**:
- Add remaining `FetcherKind` variants to `uses_tarball()`
- Implement archive URL construction for each:
  - Sourcehut: `https://git.sr.ht/~{owner}/{repo}/archive/{rev}.tar.gz`
  - Bitbucket: `https://bitbucket.org/{owner}/{repo}/get/{rev}.tar.gz`
  - Savannah: `https://git.savannah.gnu.org/cgit/{repo}.git/snapshot/{repo}-{rev}.tar.gz`
  - repo.or.cz: `https://repo.or.cz/{repo}.git/snapshot/{rev}.tar.gz`
- 9Front and Gitiles may not have archive APIs — leave as `nix-prefetch-git` fallback

**Verification**: Same as Step 1, per forge.

---

## Step 3: Pure Rust `fetchgit` hashing (shallow clone + strip `.git`)

**Status**: Not started

Replicate what `nix-prefetch-git` does for `fetchgit`: clone the repo (shallow),
checkout the requested rev, remove `.git`, NAR-serialize, SHA-256.

**Changes**:
- Add new module `src/utils/git_checkout.rs`
- Use `gix` crate (pure Rust git implementation) for shallow clone + checkout,
  or shell out to `git` as a first pass
- Implement `GitCheckout::checkout(url, rev) -> Result<PathBuf>` that returns
  a temp directory with the checked-out, `.git`-stripped tree
- Implement `GitCheckout::checkout_with_submodules(url, rev) -> Result<PathBuf>`
  that also runs `git submodule update --init --recursive`
- In `compute_git_hash()`, replace `NixPrefetcher` calls with:
  `GitCheckout::checkout()` → `hash_path()` → `NarHash`
- Keep `nix-prefetch-git` as fallback if `git` binary is not available

**Dependencies**: `gix` crate (or rely on `git` CLI)

**Verification**: Compare hashes against `nix-prefetch-git` output for several
repos. They must match exactly.

---

## Step 4: Handle `.gitattributes` export-subst/exclude for `fetchgit`

**Status**: Not started

When `fetchFromGitHub` uses `fetchgit` (via `forceFetchGit = true`), the
checkout includes files that would be excluded by `.gitattributes` export
rules. This is actually correct behavior — `fetchgit` does NOT apply export
rules, only `fetchzip`/`builtins.fetchGit` do. So this step is about ensuring
we don't accidentally apply them.

**Changes**:
- Document that the `fetchgit` path intentionally does NOT apply export rules
- Add a test verifying that a repo with `.gitattributes` export-exclude
  produces the same hash as `nix-prefetch-git` (which also doesn't apply them)

**Verification**: Test against a repo with `.gitattributes` export-subst.

---

## Step 5: `builtins.fetchGit`-compatible hashing (export-ignore aware)

**Status**: Not started

`builtins.fetchGit` with `exportIgnore = true` (the default in Nix 2.19+)
applies `.gitattributes` export rules, matching `git archive` behavior. To
compute the correct hash, we need to read the git tree and apply export
filtering.

This is the most complex step because it requires understanding git tree
objects and `.gitattributes` semantics.

**Changes**:
- Add `src/utils/git_tree.rs` module
- Use `gix` crate to read tree objects from a bare repo (no checkout needed)
- Implement export-ignore filtering by parsing `.gitattributes` entries
  in each directory level of the tree
- Implement NAR serialization of the filtered tree directly from git objects
  (no temp directory needed — stream the NAR from the tree)
- Add `hash_git_tree(url, rev, export_ignore: bool) -> Result<NarHash>`

**Dependencies**: `gix` crate

**Verification**: Compare hashes against `builtins.fetchGit` narHash output.

---

## Step 6: Remove `nix-prefetch-git` dependency entirely

**Status**: Not started

Once all fetcher kinds have pure Rust hash computation, remove the
`nix-prefetch-git` codepath.

**Changes**:
- Remove `src/utils/prefetch.rs`
- Remove `NixPrefetcher` from `src/utils/mod.rs` public exports
- Remove `nix-prefetch-git` references from `src/rules/fetcher.rs`
- Update documentation to remove `nix-prefetch-git` from requirements
- Keep `--no-prefetch` flag (still useful for skipping hash computation
  entirely in CI or offline scenarios)
- Update `flake.nix` to remove `nix-prefetch-git` from build inputs

**Verification**: Full test suite passes. Manual testing of all supported
fetcher kinds without `nix-prefetch-git` installed.

---

## Step 7: Hash format auto-detection and `nix hash convert` replacement

**Status**: Not started

Support all hash formats that Nix accepts, and provide conversion between them
without needing `nix hash convert`.

**Changes**:
- Extend `NarHash` to detect input format (SRI, nix-base32, hex)
- Add `src/utils/hash_format.rs` with bidirectional conversion:
  - SRI (`sha256-...`) ↔ nix-base32 ↔ hex
- Auto-detect the format used in the nix file and output the same format
- This replaces the need for `nix hash convert` as a separate tool

**Dependencies**: `nix-base32` crate (already added)

**Verification**: Test round-trip conversion between all three formats against
`nix hash convert` output.

---

## Dependency Summary

| Step | New crate dependencies | Removes |
|---|---|---|
| 1 | — | — |
| 2 | — | — |
| 3 | `gix` (or `git` CLI) | — |
| 4 | — | — |
| 5 | `gix` | — |
| 6 | — | `nix-prefetch-git` runtime dep |
| 7 | — | `nix hash convert` runtime dep |

## Risk Assessment

- **Steps 1-2**: Low risk. Tarball URLs are well-documented and deterministic.
- **Step 3**: Medium risk. `git clone` + strip `.git` must match `nix-prefetch-git`
  exactly. Edge cases: LFS, submodules, sparse checkouts.
- **Step 4**: Low risk. Just documentation and testing.
- **Step 5**: High risk. Git tree traversal + `.gitattributes` is complex.
  `.gitattributes` has many features (macro attributes, directory-level
  overrides, negative patterns). May need to start with a subset.
- **Step 6**: Low risk (if steps 3-5 are correct). Just cleanup.
- **Step 7**: Low risk. Pure format conversion, well-specified.
