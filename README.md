# nix-update-git

Update git references in Nix flakes and package expressions.

`nix-update-git` finds newer version tags or branch commits, shows the proposed
changes as a diff, and can apply them in place. It supports flake inputs,
standalone nixpkgs fetchers, `mkDerivation`, `buildVimPlugin`, and opt-in rules
for other package builders.

## Highlights

- Check for updates without modifying files (the default)
- Update revisions and their hashes together
- Review changes interactively, hunk by hunk
- Track branches, matching tags, or semantic-version ranges with `# follow:`
- Leave a version fixed with `# pin`
- Process individual `.nix` files or whole directories in parallel

See the [rule and pattern reference](docs/REFERENCE.md) for supported fetchers,
package builders, URL forms, and directive details.

## Installation

### Run with Nix

```bash
nix run github:yuxqiu/nix-update-git -- flake.nix
```

Or add the project to your flake inputs:

```nix
inputs.nix-update-git.url = "github:yuxqiu/nix-update-git";
```

### Install from source

```bash
cargo install --git https://github.com/yuxqiu/nix-update-git
```

The installed program requires `git` on `PATH` at runtime.

## Usage

Check a file for available updates:

```bash
nix-update-git flake.nix
```

The output is a diff and no file is changed:

```text
flake.nix
  github.com/foo/mylib
    inputs.mylib.ref
    - "v1.0.0"
    + "v2.0.0"
```

Apply all updates:

```bash
nix-update-git --update flake.nix
```

Review each update before applying it:

```bash
nix-update-git --update --interactive flake.nix
```

Interactive mode uses the familiar `git add -p` choices: `y` accepts one
hunk, `n` skips it, `a` accepts it and all remaining hunks, and `q` stops.

Files and directories can be mixed:

```bash
nix-update-git flake.nix ./packages/
```

### Selecting rules

The default rules cover flakes, standalone fetchers, `mkDerivation`, and
`buildVimPlugin`. Choose rules explicitly by repeating `--rules`, or enable
every rule with `all`:

```bash
nix-update-git \
  --rules flake \
  --rules fetcher \
  --rules build-rust-package \
  package.nix

nix-update-git --rules all package.nix
```

Some package-builder rules are opt-in because they do not update auxiliary
dependency hashes such as `cargoHash` or `vendorHash`. The
[rule reference](docs/REFERENCE.md#available-rules) lists every rule and its
default status.

### Command-line options

```text
Usage: nix-update-git [OPTIONS] [FILES_OR_DIRECTORIES]...

Arguments:
  [FILES_OR_DIRECTORIES]...  Nix files or directories containing .nix files

Options:
  -c, --check             Check without making changes (default)
  -u, --update            Perform updates
  -i, --interactive       Confirm each update
  -v, --verbose           Enable verbose output
      --color <COLOR>     Colorize diff output [default: auto] [possible values: auto, always, never]
  -j, --jobs <N>          Number of parallel file processing jobs [default: 4]
  -r, --rules <RULES>...  Rules to enable [default: flake fetcher mk-derivation build-vim-plugin]
  -h, --help              Print help
  -V, --version           Print version
```

`--color auto` (the default) colors `-`/`+` lines when stdout is a terminal and
respects `NO_COLOR`/`CLICOLOR`/`CLICOLOR_FORCE`, the same convention `git` and
`ripgrep` use.

## Documentation

- [Rule and pattern reference](docs/REFERENCE.md) — supported inputs,
  fetchers, builders, and directives
- [Contributing tests](docs/CONTRIBUTING_TESTS.md) — test layout and snapshot
  workflow

## Acknowledgments

Inspired by
[update-nix-fetchgit](https://github.com/expipiplus1/update-nix-fetchgit).
The project includes a pure-Rust implementation of `nix-prefetch-git`, so it
does not require the external `nix-prefetch-git`, `nix-hash`, or `nix-store`
executables at runtime.

## License

[MIT License](./LICENSE)
