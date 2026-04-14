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

Single-crate Rust project. Edition 2024 (requires Rust ≥ 1.85).

- `src/cli.rs` — clap CLI definition (`--check`, `--update`, `--interactive`, `--verbose`, `--format`, `--jobs`)
- `src/main.rs` — entry point; parallel check via rayon, then display/apply loop; binary-only modules: `check`, `output`, `patch`
- `src/check.rs` — `FileResult` struct + `check_file()` (read → parse → run rules)
- `src/output.rs` — `UpdateEntry` (JSON), `print_updates()`, `print_json()`, `select_interactive()`, `prompt_confirmation()`
- `src/patch.rs` — `apply_updates()` does source-level text splicing with overlap detection
- `src/parser/ast.rs` — rnix wrapper; `NixFile`, `NixNode`, `TextRange`, `NixError`; `string_content()` uses `rnix::ast::Str::normalized_parts()` for proper escape handling; `has_pin_comment()` only checks immediate sibling tokens, not recursive children
- `src/rules/fetcher/mod.rs` — `FetcherRule` struct, `FetcherCall`, `UpdateRule` impl, `extract_fetcher_calls`, `handle_branch_following`, `handle_version_update`; also handles empty hash filling via `try_fill_empty_hash` and hash prefetching via `try_prefetch_hash`; dispatches to `tarball` or `git_fetch` based on `HashStrategy`
- `src/rules/fetcher/kind.rs` — `FetcherKind` enum (all fetcher variants) and `HashStrategy` enum; methods for name lookup, URL construction, tarball/submodule detection, and hash strategy dispatch
- `src/rules/fetcher/tarball.rs` — `compute_hash()` for tarball-based fetchers (GitHub, GitLab, Codeberg); constructs tarball URLs and delegates to `TarballHasher`
- `src/rules/fetcher/git_fetch.rs` — `compute_hash()` for git-based fetchers; delegates to `NixPrefetcher` with optional submodule support
- `src/rules/flake_input.rs` — the main rule; parses flake input URLs (github:, gitlab:, sourcehut:, git+https/ssh/file) and detects version updates via `git ls-remote`
- `src/rules/traits.rs` — `UpdateRule` trait, `Update` struct (carries `TextRange` for in-place editing), `RuleRegistry`
- `src/utils/version.rs` — version comparison (`VersionDetector`); `prefix()` extracts non-numeric prefix; `latest_matching()` filters candidates by prefix shape
- `src/utils/fetch.rs` — `GitFetcher` wraps `git ls-remote`; `get_latest_tag_matching()` accepts current version for shape-aware tag selection
- `src/utils/nar.rs` — NAR serialization + SHA-256 hashing; `hash_path()` produces `NarHash` with SRI, nix-base32, and hex formats
- `src/utils/tarball.rs` — `TarballHasher` downloads + unpacks tarballs, then NAR-hashes the result
- `src/utils/prefetch.rs` — `NixPrefetcher` wraps `nix-prefetch-git` (fallback for fetchers not yet supported by pure Rust hashing)

## Key design decisions

- `Update.range` is a `TextRange { start, end }` (byte offsets into source). Update mode replaces the node's source range directly — it does NOT re-serialize the AST.
- `has_pin_comment()` checks only immediate `# pin` tokens on the node, not recursive descendants. A comment deep inside a nested attr set won't pin the parent input.
- `--check` and `--update` are mutually exclusive. The CLI exits with code 1 on parse errors or rule check errors.
- `--format json` outputs machine-readable JSON instead of human-readable text. Each entry has `file`, `rule`, `field`, `old`, `new`, `range` (byte offsets).
- `GitLocal` URLs (from `git+file://`) do resolve via `git ls-remote` on the local path. Bare relative paths (`./`, `../`) are no longer parsed — only `git+` prefixed URLs are supported.
- Inline `?ref=` in URLs is supported (e.g., `github:owner/repo?ref=v1.0`). The ref is extracted from the URL and compared as a version; on update, the entire URL string is replaced.
- `ref` vs `rev` disambiguation in branch following: if `rev` key exists → update `rev`; if `ref` key exists and the call is `builtins.fetchGit` → update `ref`; otherwise default to `rev`. Documented in `handle_branch_following` comments in `src/rules/fetcher/mod.rs`.
- The `tag` attribute is supported as a first-class update target: `tag` takes priority over `rev` in `handle_version_update`. When both `tag` and `rev` are present, `tag` is updated.
- Hash prefetching uses a dispatch strategy via `HashStrategy`: `Tarball` (GitHub/GitLab/Codeberg) uses pure Rust NAR hashing via `TarballHasher` + `hash_path()`; `Git` uses `nix-prefetch-git` via `NixPrefetcher`; `None` skips hashing (builtins.fetchGit).
- Empty hash filling: when `hash` or `sha256` is an empty string, the fetcher attempts to compute and fill the hash, even for pinned calls. `# pin` only pins the version — it does not prevent empty hash filling. Non-empty hashes on pinned calls are left untouched.
- Version shape matching: `VersionDetector::latest_matching()` filters candidate tags to those sharing the same non-numeric prefix as the current version. This prevents cross-shape updates like `v2.41 → 2.6`. The prefix is extracted by `VersionDetector::prefix()` (everything before the first digit).
- Snapshot test redaction: test `.nix` files in `tests/snapshot/data/` support a `# redact: field1 field2 ...` directive on the first line. Listed fields are omitted from the snapshot output. This allows selective redaction of non-deterministic fields like `new` and `range`.
