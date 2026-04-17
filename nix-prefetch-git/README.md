# nix-prefetch-git

A pure-Rust reimplementation of [`nix-prefetch-git`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/build-support/fetchgit/nix-prefetch-git) from nixpkgs.

Clones a git repository, makes the checkout deterministic, and computes the NAR SHA-256 hash — replacing the runtime dependency on the external `nix-prefetch-git`, `nix-hash`, and `nix-store` binaries.

## Usage

```rust
use nix_prefetch_git::{PrefetchArgs, prefetch};

let args = PrefetchArgs {
    url: "https://github.com/owner/repo".to_string(),
    rev: "v1.0.0".to_string(),
    fetch_submodules: None,
    deep_clone: None,
    leave_dot_git: None,
    fetch_lfs: None,
    branch_name: None,
    root_dir: None,
    sparse_checkout: vec![],
};

let result = prefetch(&args).unwrap();
println!("SRI hash: {}", result.sri_hash);
println!("Nix-base32 hash: {}", result.sha256_nix);
println!("Resolved rev: {}", result.rev);
```

## Supported options

| Parameter            | `PrefetchArgs` field | Description                                        |
| -------------------- | -------------------- | -------------------------------------------------- |
| `--url`              | `url`                | Git repository URL                                 |
| `--rev`              | `rev`                | Revision (tag, branch, or commit) to fetch         |
| `--fetch-submodules` | `fetch_submodules`   | Recursively fetch submodules                       |
| `--deepClone`        | `deep_clone`         | Fetch the full history instead of a shallow clone  |
| `--leave-dotGit`     | `leave_dot_git`      | Keep the `.git` directory (made deterministic)     |
| `--fetch-lfs`        | `fetch_lfs`          | Fetch Git LFS objects                              |
| `--branch-name`      | `branch_name`        | Name for the checkout branch (default: `fetchgit`) |
| `--root-dir`         | `root_dir`           | Hash a subdirectory of the checkout                |
| `--sparse-checkout`  | `sparse_checkout`    | List of paths for sparse checkout                  |

## Acknowledgments

This crate is a pure-Rust reimplementation of the [shell-based `nix-prefetch-git`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/build-support/fetchgit/nix-prefetch-git) from [nixpkgs](https://github.com/NixOS/nixpkgs). The deterministic repository cleanup logic (`.git` metadata stripping, unreachable tag removal, single-threaded repack, garbage collection) follows the same approach as the upstream shell script to produce bit-identical NAR hashes.

## License

[MIT License](./LICENSE)
