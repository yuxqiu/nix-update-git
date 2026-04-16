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

Snapshot tests use [insta](https://insta.rs/) and are defined in `tests/snapshot/`. See `docs/CONTRIBUTING_TESTS.md` for how to add new tests.

Several integration tests create temporary git repos using the `TestRepo` helper in `tests/rules/common/mod.rs`. It disables GPG signing to avoid hanging.

Network-dependent tests are gated behind the `network-tests` feature flag (`#[cfg_attr(not(feature = "network-tests"), ignore)]`). Running `cargo test` (default) skips them. To include network tests:

```bash
cargo test --features network-tests
```

Nix builds exclude network tests via `cargoTestFlags = [ "--no-default-features" ]` in `flake.nix`.

## Architecture

Workspace with two crates. Edition 2024 (requires Rust ≥ 1.85).

- `nix-prefetch-git/` — pure-Rust reimplementation of the shell-based `nix-prefetch-git` from nixpkgs; clones a git repo, makes the checkout deterministic, and computes the NAR SHA-256 hash; exposes `PrefetchArgs`, `PrefetchResult`, `NarHash`, `prefetch()`, and `nar::hash_path()`

- `src/cli.rs` — clap CLI definition (`--check`, `--update`, `--interactive`, `--verbose`, `--format`, `--jobs`)
- `src/main.rs` — entry point; parallel check via rayon, then display/apply loop; binary-only modules: `check`, `output`, `patch`
- `src/check.rs` — `FileResult` struct + `check_file()` (read → parse → run rules)
- `src/output.rs` — `UpdateEntry` (JSON), `print_updates()`, `print_json()`, `select_interactive()`, `prompt_confirmation()`
- `src/patch.rs` — `apply_updates()` does source-level text splicing with overlap detection
- `src/parser/ast.rs` — rnix wrapper; `NixFile`, `NixNode`, `TextRange`, `NixError`; `pure_string_content()` uses `rnix::ast::Str::normalized_parts()` for proper escape handling; `interpolated_string_content()` resolves simple interpolations from a variable map; `interpolated_var_affixes()` extracts prefix/suffix around a single target variable interpolation, resolving all other interpolations from a `vars` map; `has_pin_comment()` only checks immediate sibling tokens, not recursive children
- `src/rules/fetcher/mod.rs` — `FetcherRule` struct, `FetcherCall`, `UpdateRule` impl, `try_extract_call` (extracts a single fetcher call from one `NODE_APPLY`), `handle_branch_following`, `handle_version_update`; also handles empty hash filling via `try_prefetch_empty_hash` and hash prefetching via `try_prefetch_hash`; dispatches to `tarball` or `git_fetch` based on `HashStrategy`. `InterpolationSpec` controls which fetcher fields may contain interpolation: `allow()` for field-specific bindings, `allow_all()` for catch-all bindings merged on top, `allow_idents()` for bare ident resolution (e.g. `repo = pname`), and `vars_for_field()` merges `allow_all` + field-specific entries. Exposes shared helpers used by mkDerivation (`is_commit_hash`, `preferred_ref_key`, `resolve_ref_for_prefetch`, `version_ref_key_and_value`).
- `src/rules/fetcher/kind.rs` — `FetcherKind` enum (all fetcher variants) and `HashStrategy` enum; methods for name lookup, URL construction, tarball/submodule detection, and hash strategy dispatch
- `src/rules/fetcher/tarball.rs` — `compute_hash()` for tarball-based fetchers (GitHub, GitLab, Codeberg); constructs tarball URLs and delegates to `TarballHasher`
- `src/rules/fetcher/git_fetch.rs` — `compute_hash()` for git-based fetchers; builds `PrefetchArgs` from fetcher params and delegates to `nix_prefetch_git::prefetch`
- `src/rules/mk_derivation.rs` — `MkDerivationRule` handles `stdenv.mkDerivation rec { version = "..."; src = fetchX { ... }; }` patterns. It resolves source refs with precedence `tag > rev > ref` (pure, `${version}`-interpolated, or multi-variable e.g. `${pname}-${version}`), derives updates from that source ref when possible, propagates to `version`, and refreshes `hash`/`sha256` when ref/effective ref changes or hash is empty. Fetcher attributes may reference `pname` and other pure string attributes from the `mkDerivation` attrset via bare idents (e.g. `repo = pname`) or string interpolation (e.g. `owner = "${pname}-org"`) when the attrset is `rec` or lambda-wrapped; these are resolved into concrete values for URL construction and hash computation.
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
- `ref` vs `rev` disambiguation in branch following: if `rev` key exists → update `rev`; if `ref` key exists and the call is `builtins.fetchGit` → update `ref`; otherwise default to `rev`. Documented in `handle_branch_following` comments in `src/rules/fetcher/mod.rs`.
- The `tag` attribute is supported as a first-class update target: `tag` takes priority over `rev` in `handle_version_update`. When both `tag` and `rev` are present, `tag` is updated.
- Hash prefetching uses a dispatch strategy via `HashStrategy`: `Tarball` (GitHub/GitLab/Codeberg) uses pure Rust NAR hashing via `TarballHasher` + `hash_path()`; `Git` uses the built-in `nix-prefetch-git` crate; `None` skips hashing (builtins.fetchGit). No external `nix-prefetch-git` binary is required at runtime.
- Empty hash filling: when `hash` or `sha256` is an empty string, the fetcher attempts to compute and fill the hash, even for pinned calls. `# pin` only pins the version — it does not prevent empty hash filling. Non-empty hashes on pinned calls are left untouched.
- `MkDerivationRule` uses source-ref precedence `tag > rev > ref` and can operate on version refs, commit-hash refs, empty refs, and `${version}`-interpolated refs (in `rec` attrsets). It may update `version` alone (interpolated ref), `version` + source ref, and/or hash fields depending on what changed. Fetcher attributes may reference `pname` and other pure string attributes from the `mkDerivation` attrset via bare idents (e.g. `repo = pname`) or string interpolation (e.g. `owner = "${pname}-org"`) when the attrset is `rec` or lambda-wrapped; these are resolved into concrete values for URL construction and hash computation. Multi-variable source refs like `rev = "${pname}-${version}"` are supported via `interpolated_var_affixes()`.
- Fetcher attributes map to `nix_prefetch_git::PrefetchArgs` fields: `fetchSubmodules`/`submodules` → `fetch_submodules`, `deepClone` → `deep_clone`, `leaveDotGit` → `leave_dot_git`, `fetchLFS` → `fetch_lfs`, `branchName` → `branch_name`, `rootDir` → `root_dir`, `sparseCheckout` (list) → `sparse_checkout`.
- Version shape matching: `VersionDetector::latest_matching()` filters candidate tags to those sharing the same non-numeric prefix as the current version. This prevents cross-shape updates like `v2.41 → 2.6`. The prefix is extracted by `VersionDetector::prefix()` (everything before the first digit).
- Snapshot test redaction: test `.nix` files in `tests/snapshot/data/` support a `# redact: field1 field2 ...` directive on the first line. Listed fields are omitted from the snapshot output. This allows selective redaction of non-deterministic fields like `new` and `range`.
- Rule registry traversal: `RuleRegistry::check_all` performs a single AST traversal, dispatching each node to matching rules via the `matches` node-type filter. `FlakeInputRule` matches `NODE_ROOT` (needs whole-file scope to correlate `inputs.x.url` with `inputs.x.ref`). `FetcherRule` and `MkDerivationRule` match `NODE_APPLY` (process individual function call nodes — no internal tree traversal needed).
