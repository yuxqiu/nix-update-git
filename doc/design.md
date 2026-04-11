nix-update-git Design Document

## 1. Overview

`nix-update-git` is a Rust CLI tool that updates git references in Nix flake files and Nix expressions. It runs before `nix flake update` to detect and update version references in source files.

## 2. Interface

```
nix-update-git [OPTIONS] [FILES]...

Options:
  -c, --check         Check without making changes (default)
  -u, --update        Perform updates
  -i, --interactive   Confirm each update
  -j, --jobs <N>      Number of parallel update jobs (default: 4)
  -h, --help          Show help
```

Special behavior:

- Does **not** touch `flake.lock` - only updates source files
- Supports SRI hashes (`sha256-xxx`) and legacy hashes

## 3. What Needs to be Updated

### 3.1 Flake.nix Inputs

**Pattern Detection:**

- Inputs with `ref = "vX.Y.Z"` (version tags)
- Skip inputs without explicit version refs (e.g., `ref = "main"`)

**Supported Input Types:**
| Type | Detection Method |
|------|------------------|
| `github:owner/repo` | git ls-remote |
| `gitlab:owner/repo` | git ls-remote |
| `sourcehut:~user/repo` | git ls-remote |
| `git+https://...` | git ls-remote |
| `git+ssh://...` | git ls-remote
| ... | ... |

### 3.2 Nixpkgs Fetcher Calls (from update-nix-fetchgit)

| Fetcher                 | Update Type                                               |
| ----------------------- | --------------------------------------------------------- |
| `fetchgit`              | url, rev, sha256                                          |
| `fetchgitPrivate`       | url, rev, sha256, deepClone, leaveDotGit, fetchSubmodules |
| `fetchFromGitHub`       | owner, repo, rev, sha256                                  |
| `fetchFromGitLab`       | owner, repo, rev, sha256                                  |
| `fetchFromGitea`        | owner, repo, rev, sha256                                  |
| `fetchFromBitbucket`    | owner, repo, rev, sha256                                  |
| `builtins.fetchGit`     | url, rev, ref                                             |
| `builtins.fetchTarball` | url (only for GitHub archives)                            |
| `fetchurl`              | url, sha256                                               |

### 3.3 Skip Patterns

- `# pin` suffix skips any updates
- Should be implemented in every rule as where `# pin` pattern occurs is rule-dependent.

## 4. Architecture

### 4.1 Core Design: Rule-Based System

1. **Parser-agnostic**: Nix parsing isolated in `parser/` module
2. **Rule-based**: Each update type is a separate rule, pluggable
3. **Extensible**: New rules can be added without modifying core
4. **Testable**: Each component has clear interfaces
5. **Prefetch-driven**: Hash updates are calculated using `nix-prefetch-git` or internal prefetcher

```
src/
├── parser/         # Nix parsing (rnix adapter)
├── rules/          # Update rules (pluggable)
├── utils/          # Utilities for creating rules
├────  version/     # Version detection/comparison
├────  fetch/       # Remote info (GitHub, GitLab, git, ...)
├────  prefetch/    # Hash calculation (SRI, legacy)
├── main.rs         # Main: drives cli args parsing, rules running/applying/viewing (depends on modes)
└── cli.rs          # Command line definition
```

A more detailed breakdown of the envisioned architecture is shown below:

`src/`

| Module    | Responsibility          | Key Types/Functions   |
| --------- | ----------------------- | --------------------- |
| `main.rs` | CLI entry point         | `main()`, `run_cli()` |
| `cli.rs`  | Command line definition | `Cli` struct, `Args`  |

#### `src/parser/`

Responsible for: Parsing and traversing Nix expressions

| File              | Responsibility                       |
| ----------------- | ------------------------------------ |
| `mod.rs`          | Module exports                       |
| `rnix_adapter.rs` | Bridge to rnix crate, error handling |
| `ast.rs`          | Wrapper types around rnix AST        |
| `traversal.rs`    | Tree walking utilities               |

| Type       | Description                            |
| ---------- | -------------------------------------- |
| `NixFile`  | Parsed file (wrapper around rnix Root) |
| `NixNode`  | Any AST node with navigation methods   |
| `Location` | Source location (file, line, column)   |

#### `src/rules/`

Responsible for: Defining and applying update rules

| File             | Responsibility                          |
| ---------------- | --------------------------------------- |
| `mod.rs`         | Rule registry, discovers and runs rules |
| `traits.rs`      | `UpdateRule` trait, `Update` struct     |
| `flake_input.rs` | Rule for flake.nix inputs               |
| `fetchgit.rs`    | Rules for fetchgit family               |
| `fetchurl.rs`    | Rules for fetchurl/fetchzip             |
| `builtins.rs`    | Rules for builtins.fetchGit/Tarball     |

### 4.2 Rule Trait

```rust
pub trait UpdateRule {
    fn matches(&self, node: &NixNode) -> bool;
    fn check(&self, node: &NixNode) -> Result<Option<Update>>;
    fn apply(&self, node: &NixNode, update: &Update) -> Result<NixNode>;
}
```

### 4.3 Version Detection

```rust
pub trait VersionDetector: Send + Sync {
    fn is_version(&self, s: &str) -> bool;
    fn compare(&self, a: &str, b: &str) -> Ordering;
    fn latest(&self, url: &str) -> Result<String>;
}
```

### 4.4 Prefetcher Trait

```rust
pub trait HashPrefetcher: Send + Sync {
    fn prefetch_git(&self, url: &str, rev: &str) -> Result<String>;
    fn prefetch_url(&self, url: &str) -> Result<String>;
}
```

## 5. Stage Implementation

### Stage 1: Architecture Foundation (No Rules)

- Setup Rust project with rnix, clap, anyhow, tokio
- Integrate rnix-parser, create wrapper types
- CLI with argument parsing
- Test infrastructure

**Deliverable:** CLI that parses Nix files and prints AST

### Stage 2: Flake Input Rule

- Parse `inputs` block in flake.nix
- Semver/date version comparison
- Generate updated ref values

**Deliverable:** Can detect and update flake inputs with version refs

### Stage 3: Remaining Rules & Hash Prefetching

- All fetchgit variants
- builtins.fetchGit, fetchTarball
- fetchurl/fetchzip
- Hash prefetching (external calls to `nix-prefetch-git` or internal)
- SRI hash conversion
- Version attribute updates (commit dates)
- Branch/tag following from comments
- Pin handling

**Deliverable:** Full fetcher support with hash auto-update

## 6. Additional Items to Consider

1. **Nixpkgs Channel Updates** - special handling for nixpkgs input
2. **Dry Run with Diff** - show exact changes before applying
3. **Interactive Mode** - confirm each update
4. **Config File** - `.nix-updategitrc` per-project settings
5. **Git Integration** - auto-commit changes

## 7. Tech Decisions

| Choice   | Recommendation                                   |
| -------- | ------------------------------------------------ |
| Parser   | rnix (MIT, actively maintained)                  |
| Async    | tokio for parallel version checks                |
| Config   | TOML                                             |
| Errors   | anyhow for app, specific for library             |
| Prefetch | Call external `nix-prefetch-git` for correctness |

## 8. Testing Strategy

- **Git Mocks**: Create temporary local git repositories for `git ls-remote` tests.
- **Snapshot Testing**: Test rule application results against expected Nix AST snapshots.
