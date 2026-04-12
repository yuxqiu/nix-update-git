# nix-update-git

Update git references in Nix flake files and Nix expressions.

`nix-update-git` detects outdated version tags and branch commits in flake inputs and fetcher calls, then updates them in place. It also prefetches hashes via `nix-prefetch-git` so both `hash` and `sha256` stay in sync.

## Features

- **Flake inputs**: update `ref` values and inline `?ref=` in URL strings
- **Fetcher calls**: update `rev`, `tag`, and `ref` in `fetchgit`, `fetchFromGitHub`, `fetchFromGitLab`, `fetchFromGitea`, `fetchFromForgejo`, `fetchFromCodeberg`, `fetchFromSourcehut`, `fetchFromBitbucket`, `fetchFromSavannah`, `fetchFromRepoOrCz`, `fetchFrom9Front`, `fetchFromGitiles`, and `builtins.fetchGit`
- **Hash prefetching**: automatically update `hash` (SRI) and `sha256` (nix-base32) via `nix-prefetch-git`
- **Branch following**: use `# follow:<branch>` comments to track a branch's latest commit instead of version tags
- **Pinning**: `# pin` comments on any input or fetcher call skips it entirely
- **Multiple modes**: check (default), update, and interactive

## Installation

### With nix

```bash
nix run github:yuxqiu/nix-update-git
```

Or add to your flake inputs:

```nix
inputs.nix-update-git.url = "github:yuxqiu/nix-update-git";
```

### From source

```bash
cargo install --git https://github.com/yuxqiu/nix-update-git
```

Requires `git` and `nix-prefetch-git` on `$PATH` at runtime.

## Usage

```
nix-update-git [OPTIONS] [FILES_OR_DIRECTORIES]...

Options:
  -c, --check            Check without making changes (default)
  -u, --update           Perform updates
  -i, --interactive      Confirm each update
  -v, --verbose          Enable verbose output
      --format <FORMAT>  Output format: text or json [default: text]
  -h, --help             Print help
  -V, --version          Print version
```

### Check mode (default)

```bash
nix-update-git flake.nix
# flake.nix: Found 1 update(s) from rule 'flake-input':
#   - inputs.mylib.ref: v1.0.0 -> v2.0.0
```

### Update mode

```bash
nix-update-git --update flake.nix
```

### Multiple files or directories

```bash
nix-update-git flake.nix ./path/to/nix/
```

### Interactive mode

```bash
nix-update-git --update --interactive flake.nix
```

### JSON output

```bash
nix-update-git --format json flake.nix
```

Outputs machine-readable JSON:

```json
[
  {
    "file": "flake.nix",
    "rule": "flake-input",
    "field": "inputs.mylib.ref",
    "old": "\"v1.0.0\"",
    "new": "\"v2.0.0\"",
    "range": [120, 128]
  }
]
```

Combine with `--update` to apply changes and get a JSON summary of what was updated.

## Supported patterns

### Flake inputs — separate `ref`

```nix
inputs.mylib = {
  url = "github:owner/repo";
  ref = "v1.0.0";
};
```

### Flake inputs — inline `?ref=`

```nix
inputs.mylib.url = "github:owner/repo?ref=v1.0.0";
# or
inputs.mylib = "git+https://example.com/repo.git?ref=v1.0.0";
```

### Fetcher calls

```nix
src = pkgs.fetchFromGitHub {
  owner = "owner";
  repo = "repo";
  rev = "v1.0.0";
  hash = "sha256-...";
};
```

All standard nixpkgs fetchers are supported (`fetchgit`, `fetchFromGitHub`, `fetchFromGitLab`, `fetchFromGitea`, `fetchFromForgejo`, `fetchFromCodeberg`, `fetchFromSourcehut`, `fetchFromBitbucket`, `fetchFromGitiles`, `fetchFromSavannah`, `fetchFromRepoOrCz`, `fetchFrom9Front`, `builtins.fetchGit`).

### Branch following

Use `# follow: <branch>` to track a branch's latest commit instead of version tags:

```nix
src = fetchgit { # follow: master
  url = "https://github.com/owner/repo";
  rev = "e67cc2e189679f991690ade03d0ee88566d2eb0f";
  hash = "sha256-...";
};
```

### Pinned inputs

Any input or fetcher call with a `# pin` comment is skipped:

```nix
inputs.stable = { # pin
  url = "github:owner/repo";
  ref = "v1.0.0";
};
```

```nix
src = fetchFromGitHub { # pin
  owner = "owner";
  repo = "repo";
  rev = "v1.0.0";
  hash = "sha256-...";
};
```

### Supported URL types

| Type      | Example                              |
| --------- | ------------------------------------ |
| GitHub    | `github:owner/repo`                  |
| GitLab    | `gitlab:owner/repo`                  |
| SourceHut | `sourcehut:~user/repo`               |
| Git HTTPS | `git+https://example.com/repo.git`   |
| Git SSH   | `git+ssh://git@example.com/repo.git` |
| Git local | `git+file:///path/to/repo`           |

## Acknowledgments

This project was inspired by [update-nix-fetchgit](https://github.com/expipiplus1/update-nix-fetchgit), which provides similar functionality for updating fetcher calls.

## License

[MIT License](./LICENSE)
