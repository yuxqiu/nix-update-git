# nix-update-git — Future Plan

## 1. Test suite improvements

### 1.1 Parser tests (unit)

The `NixNode` / `NixFile` parser layer has almost no dedicated unit tests. Add a `src/parser/tests.rs` module covering:

- `string_content` for double-quoted, indented strings, and edge cases (empty string, escape sequences)
- `attrpath_segments` for simple and dotted paths
- `has_pin_comment` and `follow_branch_comment` with various comment placements
- `apply_function_name` for simple idents, dotted names (`pkgs.fetchFromGitHub`), and nested applies
- `find_string_value` / `find_bool_value` correctness

### 1.2 Rule-internal logic tests (unit)

Both `FetcherRule` and `FlakeInputRule` have large `check()` methods that call out to `git ls-remote` and `nix-prefetch-git`. The pure-logic parts (URL reconstruction, version comparison, pin detection, `# follow:` parsing) can and should be tested without subprocesses by testing the internal helper methods more thoroughly:

- `FlakeInputRule::parse_flake_url` — already has good coverage; add edge cases (trailing slashes on GitHub URLs, ref-less git+https URLs, nested query params like `?ref=v1&rev=abc`)
- `FlakeInputRule::reconstruct_url` — test round-trip: `parse → reconstruct → parse` is stable
- `FetcherKind::git_url` — already unit-tested; add `BuiltinsFetchGit` and `FetchGit` with `url` param
- `FetcherKind::uses_fetch_submodules` — test `fetchSubmodules=true`, `forceFetchGit=true`, and `false`/absence

### 1.3 Integration test coverage gaps

Current integration tests (in `tests/`) cover the main flows but miss several patterns:

- **`fetchFromGitLab` / `fetchFromGitea` / `fetchFromForgejo` / `fetchFromCodeberg` / `fetchFromSavannah` / `fetchFromRepoOrCz` / `fetchFrom9Front` / `fetchFromGitiles`**: tested only as parse-only (no update assertion). Add local-git-repo-backed tests that verify `rev` updates, like the existing `fetchgit` and `fetchFromGitHub` tests.
- **`builtins.fetchGit` with `rev` instead of `ref`**: currently untested — `builtins.fetchGit` supports `rev` too; should verify it's handled.
- **Multiple fetcher calls in one file**: test that overlapping or independent updates are all detected.
- **Overlapping update ranges**: the `apply_updates` logic in `main.rs` that detects subrange overlaps should have a direct test.
- **`# pin` on inner attributes**: e.g., `rev = "v1.0.0"; # pin` — confirm pin detection on non-outermost nodes.

### 1.4 Hermetic test infrastructure

Current integration tests rely on `git init` + `git tag` in temp directories, which is good. Some tests hit the real network (e.g., `test_github_fetch_from_github_detects_update` queries `github.com/yuxqiu/nix-update-git` tags). Consider:

- Making network-dependent tests opt-in (e.g., `#[cfg(feature = "network-tests")]`) so `cargo test` works offline.
- Creating a more systematic `TestRepo` builder that sets up known tag structures, branches, and commits.

## 2. Architecture and rule improvements

### 2.1 `fetchFromGitHub` with `tag` attribute

Currently the fetcher rule only updates `rev` and `tag` if the current value looks like a version. But `fetchFromGitHub { ...; tag = "v1.0.0"; ... }` is a legitimate pattern where `tag` is a separate attribute that should be updated. Already partially handled — verify it works end-to-end with integration tests.

### 2.2 `--dry-run` output format

Currently check mode prints human-readable text. Consider adding `--format json` for machine-readable output:

```json
[
  {
    "file": "flake.nix",
    "rule": "flake-input",
    "field": "inputs.mylib.ref",
    "old": "v1.0.0",
    "new": "v2.0.0",
    "range": [120, 127]
  }
]
```

This enables integration with editor tooling and CI systems.

### 2.3 `nix-prefetch-git` fallback strategy

If `nix-prefetch-git` is not available, the tool currently just prints a warning and skips hash updates. Consider:

- Auto-detecting availability at startup and informing the user.
- Providing a `--no-prefetch` flag to explicitly skip hash prefetching (useful in CI where `nix-prefetch-git` might not be installed).
- Supporting `nix hash convert` as an alternative for SRI ↔ nix-base32 conversion.

### 2.4 `ref` vs `rev` disambiguation

The `handle_branch_following` method currently picks between `"rev"` and `"ref"` keys with a somewhat ad-hoc heuristic. The logic should be:

- If `rev` key exists in the call → update `rev`
- If `ref` key exists in the call → update `ref`
- For `builtins.fetchGit`, prefer `ref` (since `ref` is the standard attribute for branch names)

This is already what the code does, but it should be clearly documented in comments.

## 3. Code quality

### 3.1 Error reporting

Currently errors are printed to stderr with `eprintln!` in various places. Consider:

- Using `anyhow` consistently at the top level (`process_file` already returns `Result`).
- A `--quiet` flag to suppress warnings.
- Structured error codes (exit 0 = no updates, exit 1 = error, exit 2 = updates found in check mode).

### 3.2 Better `string_content` handling

The current `string_content` method strips quotes manually (`text[1..len-1]`), which doesn't handle escape sequences like `\"` or `\n` inside strings. For the current use case (URLs, refs, hashes) this is fine, but a more robust approach would use the rnix AST's string token API.

## 4. Performance

### 4.1 Parallel file processing

Currently files are processed sequentially. For large repos with many `.nix` files, processing them in parallel with `rayon` or `std::thread::scope` would be a significant speedup.
