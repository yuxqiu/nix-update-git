# AGENTS.md

## Commands

```bash
cargo fmt --all -- --check    # format check (must pass in CI)
cargo clippy --all-targets -- -D warnings  # lint (warnings are errors)
cargo test                     # all tests (unit + integration)
```

There is no separate typecheck or lint step — clippy covers it.

## Test suites

Unit tests live in `src/**/tests` modules. Integration tests live in `tests/rules/*.rs`.

Snapshot tests use [insta](https://insta.rs/) and are defined in `tests/snapshot/`. They run as a separate test binary (`--test snapshot`) with a custom harness (`libtest_mimic`) that registers each `.nix` file as an individual test case, giving per-file progress in `cargo test` output. See `docs/CONTRIBUTING_TESTS.md` for how to add new tests.

Several integration tests create temporary git repos using the `TestRepo` helper in `tests/rules/common/mod.rs`. It disables GPG signing to avoid hanging.

Network-dependent snapshot tests are automatically ignored when the `network-tests` feature is not enabled (via `libtest_mimic`'s `with_ignored_flag`). Other network-dependent unit/integration tests use `#[cfg_attr(not(feature = "network-tests"), ignore)]`. Running `cargo test` (default) skips them. To include network tests:

```bash
cargo test --features network-tests
```

To run only snapshot tests:

```bash
cargo test --features network-tests --test snapshot
```

To run only non-snapshot integration tests:

```bash
cargo test --test mod
```

Nix builds exclude network tests via `cargoTestFlags = [ "--no-default-features" ]` in `flake.nix`.

## Adding snapshot tests

1. Add a `.nix` file in `tests/snapshot/data/<category>/<test_name>.nix`
2. Verify the Nix expression evaluates correctly with nixpkgs:
   ```bash
   nix build -L --impure --expr '
   let
     wrapped = builtins.toFile "wrapped.nix" (
       "let pkgs = import <nixpkgs> {}; in\n"
       + builtins.readFile ./tests/snapshot/data/<category>/<test_name>.nix
     );
   in
     (import wrapped).src
   '
   ```
   Adjust the attribute path (`.src`, `.patch`, etc.) to match what the Nix file exports.
3. Run `cargo test --features network-tests --test snapshot` to generate the snapshot. New snapshots land in `tests/snapshot/snaps/<category>/<test_name>.snap`.
4. To update existing snapshots: `INSTA_UPDATE=always cargo test --features network-tests --test snapshot`.
5. Add `# redact: field1 field2 ...` on the first line to omit non-deterministic fields (e.g. `new`, `range`) from the snapshot.

## Architecture

Workspace with two crates. Edition 2024 (requires Rust ≥ 1.85).

- `nix-prefetch-git/` — pure-Rust reimplementation of the shell-based `nix-prefetch-git` from nixpkgs; clones a git repo, makes the checkout deterministic, and computes the NAR SHA-256 hash; exposes `PrefetchArgs`, `PrefetchResult`, `NarHash`, `prefetch()`, and `nar::hash_path()`

- `src/cli.rs` — clap CLI definition (`--check`, `--update`, `--interactive`, `--verbose`, `--format`, `--jobs`, `--rules`)
- `src/main.rs` — entry point; parallel check via rayon, then display/apply loop; registers rules based on `--rules` flag; binary-only modules: `check`, `output`, `patch`
- `src/check.rs` — `FileResult` struct + `check_file()` (read → parse → run rules)
- `src/output.rs` — `UpdateEntry` (JSON), `print_updates()`, `print_json()`, `select_interactive()`, `prompt_confirmation()`
- `src/patch.rs` — `apply_updates()` does source-level text splicing with overlap detection
- `src/parser/ast.rs` — rnix wrapper; `NixFile`, `NixNode`, `TextRange`, `NixError`; `pure_string_content()` uses `rnix::ast::Str::normalized_parts()` for proper escape handling; `interpolated_string_content()` resolves simple interpolations from a variable map; `interpolated_var_affixes()` extracts prefix/suffix around a single target variable interpolation, resolving all other interpolations from a `vars` map; `has_pin_comment()` only checks immediate sibling tokens, not recursive children; `parse_attrs()` centralizes attr-set parsing with typed output (`ParsedAttrs`: strings, bools, ints, list_strings, list_ints, source_ranges, unknown_keys)
- `src/rules/fetcher/mod.rs` — `FetcherRule` struct, `FetcherCall`, `UpdateRule` impl, `try_extract_call` (extracts a single fetcher call from one `NODE_APPLY`), `handle_following`, `handle_version_update`; `FollowSpec` enum (`Branch`, `Regex`, `Semver`) determines how `# follow:` directives resolve: `# follow:branch <name>` follows a branch tip, `# follow:regex <pattern>` full-matches tags against a regex, `# follow:semver <requirement>` matches tags against a semver version requirement after stripping each tag's version prefix (e.g. `v` from `v1.0.0`); `parse_follow_spec()` parses the directive text; `resolve_follow()` dispatches resolution; also handles empty hash filling via `try_prefetch_empty_hash` and hash prefetching via `try_prefetch_hash`; dispatches to `tarball` or `git_fetch` based on `HashStrategy`. `InterpolationSpec` controls which fetcher fields may contain interpolation: `allow()` for field-specific bindings, `allow_all()` for catch-all bindings merged on top, `allow_idents()` for bare ident resolution (e.g. `repo = pname`), and `vars_for_field()` merges `allow_all` + field-specific entries. Exposes shared helpers used by derivation rules (`is_commit_hash`, `preferred_ref_key`, `resolve_ref_for_prefetch`, `version_ref_key_and_value`). `is_src_of_owned_call` unconditionally skips `src =` inside any function listed in `OWNED_FUNC_NAMES` (all nixpkgs derivation-wrapper functions), preventing the fetcher rule from conflicting with derivation rules.
- `src/rules/derivation/core.rs` — `DerivationRule` struct (generic derivation rule parameterized by rule name and function name list); handles `version` + `src` patterns like `rec { version = "..."; src = fetchX { ... }; }`; resolves source refs with precedence `tag > rev > ref`; supports pure, interpolated (`${version}`), ident-from-version, and multi-variable source refs; refreshes `hash`/`sha256` when needed; uses `InterpolationSpec` for resolving bare idents and string interpolations from the attrset.
- `src/rules/derivation/mod.rs` — exports `DerivationRule`, `OWNED_FUNC_NAMES` (canonical list of all nixpkgs derivation-wrapper function names that the fetcher rule should skip), and factory functions for each rule (`mk_derivation_rule`, `build_rust_package_rule`, etc.). Language-grouped rules accept multiple function names: `build-go-module` handles both `buildGoModule` and `buildGoPackage`; `build-python-package` handles both `buildPythonPackage` and `buildPythonApplication`; `build-haskell-package` handles both `buildHaskellPackage` and `mkHaskellPackage`.
- `src/rules/fetcher/kind.rs` — `FetcherKind` enum (all fetcher variants including `FetchPatch`) and `HashStrategy` enum; per-kind `attr_spec()` returns the typed attribute schema; `operational_keys()` derives from the spec; methods for name lookup, URL construction, tarball/submodule detection, and hash strategy dispatch
- `src/rules/fetcher/tarball.rs` — `compute_hash()` for tarball-based fetchers (GitHub, GitLab, Gitea, Forgejo, Codeberg, SourceHut, Bitbucket, Gitiles, RepoOrCz); constructs tarball URLs and delegates to `TarballHasher`
- `src/rules/fetcher/git_fetch.rs` — `compute_hash()` for git-based fetchers; builds `PrefetchArgs` from fetcher params and delegates to `nix_prefetch_git::prefetch`
- `src/rules/flake_input.rs` — the main rule; parses flake input URLs (github:, gitlab:, sourcehut:, git+https/ssh/file) and detects version updates via `git ls-remote`
- `src/rules/traits.rs` — `UpdateRule` trait (with `matches` node-type filter and `check` per-node processing), `Update` struct (carries `TextRange` for in-place editing), `RuleRegistry` (single AST traversal dispatching each node to matching rules)
- `src/utils/version.rs` — version comparison (`VersionDetector`); `prefix()` extracts non-numeric prefix; `latest_matching()` filters candidates by prefix shape
- `src/utils/fetch.rs` — `GitFetcher` wraps `git ls-remote`; `get_latest_tag_matching()` accepts current version for shape-aware tag selection
- `src/utils/tarball.rs` — `TarballHasher` downloads + unpacks tarballs, then NAR-hashes the result via `nix_prefetch_git::nar::hash_path`

## Key design decisions

- `Update.range` is a `TextRange { start, end }` (byte offsets into source). Update mode replaces the node's source range directly — it does NOT re-serialize the AST.
- `has_pin_comment()` checks only immediate `# pin` tokens on the node, not recursive descendants. A comment deep inside a nested attr set won't pin the parent input.
- `--check` and `--update` are mutually exclusive. The CLI exits with code 1 on parse errors or rule check errors.
- `--format json` outputs machine-readable JSON instead of human-readable text. Each entry has `file`, `rule`, `field`, `old`, `new`, `range` (byte offsets).
- `GitLocal` URLs (from `git+file://`) do resolve via `git ls-remote` on the local path. Bare relative paths (`./`, `../`) are no longer parsed — only `git+` prefixed URLs are supported.
- Inline `?ref=` in URLs is supported (e.g., `github:owner/repo?ref=v1.0`). The ref is extracted from the URL and compared as a version; on update, the entire URL string is replaced.
- `ref` vs `rev` disambiguation in branch following: if `rev` key exists → update `rev`; if `ref` key exists and the call is `builtins.fetchGit` → update `ref`; otherwise default to `rev`. Documented in `handle_following` comments in `src/rules/fetcher/mod.rs`.
- The `tag` attribute is supported as a first-class update target: `tag` takes priority over `rev` in `handle_version_update`. When both `tag` and `rev` are present, `tag` is updated.
- Hash prefetching uses a dispatch strategy via `HashStrategy`: `Tarball` (GitHub, GitLab, Gitea, Forgejo, Codeberg, SourceHut, Bitbucket, Gitiles, RepoOrCz) uses pure Rust NAR hashing via `TarballHasher` + `hash_path()`; `Git` uses the built-in `nix-prefetch-git` crate; `None` skips hashing (builtins.fetchGit). No external `nix-prefetch-git` binary is required at runtime.
- Empty hash filling: when `hash` or `sha256` is an empty string, the fetcher attempts to compute and fill the hash, even for pinned calls. `# pin` only pins the version — it does not prevent empty hash filling. Non-empty hashes on pinned calls are left untouched.
- `MkDerivationRule` uses source-ref precedence `tag > rev > ref` and can operate on version refs, commit-hash refs, empty refs, and `${version}`-interpolated refs (in `rec` attrsets). It may update `version` alone (interpolated ref), `version` + source ref, and/or hash fields depending on what changed. Fetcher attributes may reference `pname` and other pure string attributes from the `mkDerivation` attrset via bare idents (e.g. `repo = pname`) or string interpolation (e.g. `owner = "${pname}-org"`) when the attrset is `rec` or lambda-wrapped; these are resolved into concrete values for URL construction and hash computation. Multi-variable source refs like `rev = "${pname}-${version}"` are supported via `interpolated_var_affixes()`.
- `DerivationRule` is a generic rule parameterized by a rule name and a list of nixpkgs function names. Each derivation rule (`mk-derivation`, `build-rust-package`, `build-go-module`, etc.) is a `DerivationRule` instance created via factory functions in `src/rules/derivation/mod.rs`. Language-grouped rules accept multiple function names (e.g. `build-go-module` handles `buildGoModule` and `buildGoPackage`). Default-enabled rules: `fetcher`, `flake`, `mk-derivation`. All other derivation rules are off by default because they may have additional dependencies (e.g. `cargoHash`, `vendorHash`) that this tool does not update.
- `OWNED_FUNC_NAMES` in `src/rules/derivation/mod.rs` lists all nixpkgs derivation-wrapper function names. The fetcher rule uses this list unconditionally via `is_src_of_owned_call` to skip `src =` attributes inside any of these functions, preventing conflicts with derivation rules regardless of whether a specific derivation rule is enabled.
- `--rules` flag configures which rules to enable. `RuleName::rule_id()` returns the hyphen-separated identifier used in `is_enabled()` checks. `all` enables every rule.
- Fetcher attributes map to `nix_prefetch_git::PrefetchArgs` fields: `fetchSubmodules`/`submodules` → `fetch_submodules`, `deepClone` → `deep_clone`, `leaveDotGit` → `leave_dot_git`, `fetchLFS` → `fetch_lfs`, `branchName` → `branch_name`, `rootDir` → `root_dir`, `sparseCheckout` (list) → `sparse_checkout`.
- Version shape matching: `VersionDetector::latest_matching()` filters candidate tags to those sharing the same non-numeric prefix as the current version. This prevents cross-shape updates like `v2.41 → 2.6`. The prefix is extracted by `VersionDetector::prefix()` (everything before the first digit).
- Snapshot test redaction: test `.nix` files in `tests/snapshot/data/` support a `# redact: field1 field2 ...` directive on the first line. Listed fields are omitted from the snapshot output. This allows selective redaction of non-deterministic fields like `new` and `range`.
- Rule registry traversal: `RuleRegistry::check_all` performs a single AST traversal, dispatching each node to matching rules via the `matches` node-type filter. `FlakeInputRule` matches `NODE_ROOT` (needs whole-file scope to correlate `inputs.x.url` with `inputs.x.ref`). `FetcherRule` and all `DerivationRule` instances match `NODE_APPLY` (process individual function call nodes — no internal tree traversal needed).
