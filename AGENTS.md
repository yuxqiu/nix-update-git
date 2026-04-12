# AGENTS.md

## Commands

```bash
cargo fmt --all -- --check    # format check (must pass in CI)
cargo clippy --all-targets -- -D warnings  # lint (warnings are errors)
cargo test                     # all tests (unit + integration)
```

There is no separate typecheck or lint step — clippy covers it.

## Test suites

Unit tests live in `src/**/tests` modules. Integration tests live in `tests/*.rs`.

Several integration tests create temporary git repos using the `TestRepo` helper in `tests/common/mod.rs`. It disables GPG signing to avoid hanging.

Network-dependent tests are gated behind the `network-tests` feature flag (`#[cfg(feature = "network-tests")]`). Running `cargo test` (default) skips them. To include network tests:

```bash
cargo test --features network-tests
```

Nix builds exclude network tests via `cargoTestFlags = [ "--no-default-features" ]` in `flake.nix`.

## Architecture

Single-crate Rust project. Edition 2024 (requires Rust ≥ 1.85).

- `src/cli.rs` — clap CLI definition (`--check`, `--update`, `--interactive`, `--verbose`, `--format`)
- `src/main.rs` — entry point; check/update/interactive/json modes; `apply_updates()` does source-level text splicing
- `src/parser/ast.rs` — rnix wrapper; `NixFile`, `NixNode`, `TextRange`, `NixError`; `string_content()` uses `rnix::ast::Str::normalized_parts()` for proper escape handling; `has_pin_comment()` only checks immediate sibling tokens, not recursive children
- `src/rules/flake_input.rs` — the main rule; parses flake input URLs (github:, gitlab:, sourcehut:, git+https/ssh/file) and detects version updates via `git ls-remote`
- `src/rules/traits.rs` — `UpdateRule` trait, `Update` struct (carries `TextRange` for in-place editing), `RuleRegistry`
- `src/utils/version.rs` — semver version comparison (`VersionDetector`)
- `src/utils/fetch.rs` — `GitFetcher` wraps `git ls-remote`

## Key design decisions

- `Update.range` is a `TextRange { start, end }` (byte offsets into source). Update mode replaces the node's source range directly — it does NOT re-serialize the AST.
- `has_pin_comment()` checks only immediate `# pin` tokens on the node, not recursive descendants. A comment deep inside a nested attr set won't pin the parent input.
- `--check` and `--update` are mutually exclusive. The CLI exits with code 1 on parse errors or rule check errors.
- `--format json` outputs machine-readable JSON instead of human-readable text. Each entry has `file`, `rule`, `field`, `old`, `new`, `range` (byte offsets).
- `GitLocal` URLs (from `git+file://`) do resolve via `git ls-remote` on the local path. Bare relative paths (`./`, `../`) are no longer parsed — only `git+` prefixed URLs are supported.
- Inline `?ref=` in URLs is supported (e.g., `github:owner/repo?ref=v1.0`). The ref is extracted from the URL and compared as a version; on update, the entire URL string is replaced.