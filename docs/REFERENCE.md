# Rule and pattern reference

Detailed reference for what `nix-update-git` recognizes and how each directive behaves. See the [README](../README.md) for installation and basic usage.

## Rules

`nix-update-git` uses rules to detect and apply updates. Each rule targets a specific pattern. By default, `fetcher`, `flake`, `mk-derivation`, and `build-vim-plugin` are enabled. Additional derivation rules can be enabled via `--rules`:

```bash
# Select the default rules plus buildRustPackage
nix-update-git --rules fetcher --rules flake --rules mk-derivation \
  --rules build-vim-plugin --rules build-rust-package file.nix

# Enable all rules
nix-update-git --rules all file.nix
```

### Available rules

| Rule | Default | Nix function names | Description |
| --- | --- | --- | --- |
| `fetcher` | yes | — | Standalone fetcher calls (`fetchgit`, `fetchFromGitHub`, etc.) |
| `flake` | yes | — | Flake input URLs and refs |
| `mk-derivation` | yes | `mkDerivation` | `stdenv.mkDerivation rec { version = ...; src = fetchX { ... }; }` |
| `build-vim-plugin` | yes | `vimUtils.buildVimPlugin` | Vim/Neovim plugins |
| `build-rust-package` | no | `buildRustPackage` | Rust packages (note: does not update `cargoHash`/`cargoSha256`) |
| `build-go-module` | no | `buildGoModule`, `buildGoPackage` | Go modules (note: does not update `vendorHash`) |
| `build-python-package` | no | `buildPythonPackage`, `buildPythonApplication` | Python packages |
| `build-dune-package` | no | `buildDunePackage` | OCaml/Dune packages |
| `build-npm-package` | no | `buildNpmPackage` | Node.js packages |
| `build-mix-package` | no | `buildMixPackage` | Elixir packages |
| `build-rebar3-release` | no | `buildRebar3Release` | Erlang packages |
| `build-gem` | no | `buildGem` | Ruby gems |
| `build-haskell-package` | no | `buildHaskellPackage`, `mkHaskellPackage` | Haskell packages |
| `build-emscripten-package` | no | `buildEmscriptenPackage` | Emscripten packages |

The `fetcher` rule never processes `src =` attributes inside any of the derivation-wrapper functions above — the derivation rules handle those exclusively. Enabling a derivation rule is needed for version updates in those patterns; the fetcher rule handles standalone fetcher calls regardless.

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

Supported fetcher functions are `fetchgit`, `fetchFromGitHub`, `fetchFromGitLab`, `fetchFromGitea`, `fetchFromForgejo`, `fetchFromCodeberg`, `fetchFromSourcehut`, `fetchFromBitbucket`, `fetchFromGitiles`, `fetchFromRepoOrCz`, `fetchpatch`, `fetchTarball`, and `builtins.fetchGit`.

### mkDerivation

`mkDerivation` updates `version` together with the source ref in `src` (priority: `tag` > `rev` > `ref`) and refreshes `hash`/`sha256` when needed.

Supported source-ref behaviors:

- Pure version ref equal to `version` (for example `rev = "v1.0.0"`) updates both together.
- Pure commit-hash ref uses `version` to find newer upstream tags, then updates `version` and the ref.
- Empty source ref can be populated from `version`.
- Interpolated source refs that depend on `${version}` (in `rec` attrsets) update `version`; the interpolated ref text stays as-is.
- Interpolated source refs that combine `${pname}` and `${version}` (for example `rev = "${pname}-${version}"`) update `version`; the interpolated ref text stays as-is.

Fetcher attributes may also reference `pname` and other pure string attributes from the `mkDerivation` attrset via bare idents or string interpolation, when the attrset is `rec` or lambda-wrapped:

```nix
stdenv.mkDerivation rec {
  pname = "my-package";
  version = "1.0.0";
  src = fetchFromGitHub {
    owner = "my-org";
    repo = pname;                    # bare ident
    rev = "v${version}";
    hash = "sha256-...";
  };
};
```

```nix
stdenv.mkDerivation (finalAttrs: {
  pname = "my-package";
  version = "1.0.0";
  src = fetchFromGitHub {
    owner = "${finalAttrs.pname}-org";  # dotted interpolation
    repo = finalAttrs.pname;            # bare ident
    rev = "v${finalAttrs.version}";
    hash = "sha256-...";
  };
});
```

Any pure string attribute (not just `pname`) from the `mkDerivation` attrset can be referenced this way. Without `rec` or a lambda wrapper, variable references in the fetcher are not resolved and the call is skipped.

```nix
stdenv.mkDerivation rec {
  name = "foo-${version}";
  version = "1.0.0";
  src = fetchgit {
    url = "https://github.com/owner/repo";
    rev = "e67cc2e189679f991690ade03d0ee88566d2eb0f";
    sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
  };
};
```

When the `# pin` comment is present on the `mkDerivation` call, the derivation rule skips the entire block.

### Branch following

Use `# follow:branch <name>` to track a branch's latest commit instead of version tags:

```nix
src = fetchgit { # follow:branch master
  url = "https://github.com/owner/repo";
  rev = "e67cc2e189679f991690ade03d0ee88566d2eb0f";
  hash = "sha256-...";
};
```

The `# follow:` directive supports three modes:

| Mode   | Syntax                          | Behavior                                                                                                                        |
| ------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Branch | `# follow:branch <name>`        | Tracks the latest commit on the given branch                                                                                    |
| Regex  | `# follow:regex <pattern>`      | Finds the latest tag matching `^<pattern>$` (full match) and resolves it to a SHA                                               |
| Semver | `# follow:semver <requirement>` | Finds the latest tag whose version (after stripping prefix like `v`) satisfies the semver requirement, and resolves it to a SHA |

Examples:

```nix
# Follow the main branch
src = fetchgit { # follow:branch main
  url = "https://github.com/owner/repo";
  rev = "0000000000000000000000000000000000000000";
  hash = "sha256-...";
};

# Follow tags matching a regex (full match)
src = fetchgit { # follow:regex v[0-9]+\.[0-9]+\.[0-9]+
  url = "https://github.com/owner/repo";
  rev = "0000000000000000000000000000000000000000";
  hash = "sha256-...";
};

# Follow tags within a semver range (prefix like 'v' is auto-stripped)
src = fetchFromGitHub { # follow:semver ^0.1
  owner = "owner";
  repo = "repo";
  rev = "v0.1.0";
  hash = "sha256-...";
};

# Only allow updates within 0.x
src = fetchFromGitHub { # follow:semver <1.0.0
  owner = "owner";
  repo = "repo";
  rev = "v0.5.0";
  hash = "sha256-...";
};

# fetchpatch also supports follow directives
patches = [ (fetchpatch { # follow:branch main
  url = "https://github.com/owner/repo/commit/abc123.patch";
  hash = "";
}) ];
```

### Pinned inputs

`# pin` prevents version updates and `# follow:` resolution. For standalone fetcher calls, an empty `hash` or `sha256` is still filled; an existing non-empty hash is left untouched. A pinned derivation rule is skipped entirely.

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
