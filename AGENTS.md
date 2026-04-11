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

Several integration tests create temporary git repos via shell commands. They must disable GPG signing to avoid hanging:

```bash
git config commit.gpgsign false
git config tag.gpgsign false
```

The `create_git_repo_with_tags` helper in `tests/flake_input_test.rs` handles this already, but keep it in mind if writing new git-dependent tests.

Nix builds skip these tests (see `checkFlags` in `flake.nix`).

## Architecture

Single-crate Rust project. Edition 2024 (requires Rust ≥ 1.85).

- `src/cli.rs` — clap CLI definition (`--check`, `--update`, `--interactive`)
- `src/main.rs` — entry point; check/update/interactive modes; `apply_updates()` does source-level text splicing
- `src/parser/ast.rs` — rnix wrapper; `NixFile`, `NixNode`, `TextRange`, `NixError`; `has_pin_comment()` only checks immediate sibling tokens, not recursive children
- `src/rules/flake_input.rs` — the main rule; parses flake input URLs (github:, gitlab:, sourcehut:, git+https/ssh/file) and detects version updates via `git ls-remote`
- `src/rules/traits.rs` — `UpdateRule` trait, `Update` struct (carries `TextRange` for in-place editing), `RuleRegistry`
- `src/utils/version.rs` — semver version comparison (`VersionDetector`)
- `src/utils/fetch.rs` — `GitFetcher` wraps `git ls-remote`

## Key design decisions

- `Update.range` is a `TextRange { start, end }` (byte offsets into source). Update mode replaces the node's source range directly — it does NOT re-serialize the AST.
- `has_pin_comment()` checks only immediate `# pin` tokens on the node, not recursive descendants. A comment deep inside a nested attr set won't pin the parent input.
- `--check` and `--update` are mutually exclusive. The CLI exits with code 1 on parse errors or rule check errors.
- `GitLocal` URLs (from `git+file://`) do resolve via `git ls-remote` on the local path. Bare relative paths (`./`, `../`) are no longer parsed — only `git+` prefixed URLs are supported.
- Inline `?ref=` in URLs is supported (e.g., `github:owner/repo?ref=v1.0`). The ref is extracted from the URL and compared as a version; on update, the entire URL string is replaced.