# nix-update-git

Update git references in Nix flake files and Nix expressions.

`nix-update-git` detects outdated version tags in flake inputs and fetcher calls, then updates them in place.

## Features

- Detect and update flake input refs (`ref = "v1.0.0"` → `ref = "v2.0.0"`)
- Detect and update inline refs (`url = "github:owner/repo?ref=v1.0.0"`)
- Supports `github:`, `gitlab:`, `sourcehut:`, `git+https://`, `git+ssh://`, `git+file://` URLs
- Pin inputs with `# pin` — they will be skipped entirely
- Check mode (default) shows what would change without modifying files
- Update mode applies changes in place
- Interactive mode prompts before each change

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

## Usage

```
nix-update-git [OPTIONS] [FILES]...

Options:
  -c, --check         Check without making changes (default)
  -u, --update        Perform updates in place
  -i, --interactive   Confirm each update before applying
  -h, --help          Show help
```

### Check mode (default)

```bash
nix-update-git flake.nix
# output:
# flake.nix: Found 1 update(s) from rule 'flake-input':
#   - inputs.mylib.ref: v1.0.0 -> v2.0.0
```

### Update mode

```bash
nix-update-git --update flake.nix
# Applies changes to flake.nix in place
```

### Interactive mode

```bash
nix-update-git --update --interactive flake.nix
# Prompts y/N for each update
```

## Supported patterns

### Flake inputs — separate `ref`

```nix
inputs.mylib = {
  url = "github:owner/repo";
  ref = "v1.0.0";  # updated to latest tag
};
```

### Flake inputs — inline `?ref=`

```nix
inputs.mylib.url = "github:owner/repo?ref=v1.0.0";
# or
inputs.mylib = "git+https://example.com/repo.git?ref=v1.0.0";
```

### Pinned inputs

Any input with a `# pin` comment is skipped:

```nix
inputs.stable = { # pin
  url = "github:owner/repo";
  ref = "v1.0.0";
};
```

```nix
inputs.stable.ref = "v1.0.0"; # pin
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

## Roadmap

This is stage 2 of the planned implementation. Remaining work:

- [ ] `fetchgit` / `fetchFromGitHub` / other nixpkgs fetcher rules
- [ ] `builtins.fetchGit` / `builtins.fetchTarball` rules
- [ ] Hash prefetching (`nix-prefetch-git` or internal)
- [ ] SRI hash conversion

## License

[MIT License](./LICENSE)
