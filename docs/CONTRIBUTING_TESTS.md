# Contributing Tests with Insta

This project uses [insta](https://insta.rs/) for snapshot testing. Tests are defined in `tests/snapshot/`.

Each `.nix` file in `data/` is registered as an individual test case (e.g. `fetcher/arkenfox_user_js_hash`), so `cargo test` reports per-file progress without needing `--nocapture`.

## Writing a New Test

1. **Add a Nix file** in `tests/snapshot/data/<category>/<test_name>.nix`

2. **Run the snapshot tests** to generate the snapshot:

   ```bash
   cargo test --features network-tests --test snapshot
   ```

   New snapshots are created automatically on the first run. To update existing snapshots, set the `INSTA_UPDATE` environment variable:

   ```bash
   INSTA_UPDATE=always cargo test --features network-tests --test snapshot
   ```

   For review mode (opens your editor for each changed snapshot):

   ```bash
   INSTA_UPDATE=new cargo test --features network-tests --test snapshot
   ```

3. **Verify the snapshot** was created at `tests/snapshot/snaps/<category>/<test_name>.snap`

## Test Structure

Nix files in `data/` are processed by the custom test harness in `tests/snapshot/main.rs`:

- Each `.nix` file is discovered and registered as an individual test case via `libtest_mimic`
- Each file is run through `nix-update-git --format json`
- JSON output is parsed into `SnapshotEntry` structs (rule, field, old, new, range)
- The result is snapshot under `snaps/<category>/<test_name>.snap`

### Redacting Sensitive Values

Add a `# redact: field1 field2 ...` directive on the first line of the `.nix` file. Listed fields are omitted from the snapshot output. This allows selective redaction of non-deterministic fields like `new` and `range`.

## Updating Existing Snapshots

```bash
INSTA_UPDATE=always cargo test --features network-tests --test snapshot
```

Use `INSTA_UPDATE=new` to only create new snapshots without updating existing ones.

## Running Tests

```bash
cargo test                     # skip network/snapshot tests (default)
cargo test --features network-tests --test snapshot  # run snapshot tests
```

To run a single snapshot test, filter by name:

```bash
cargo test --features network-tests --test snapshot -- fetcher/arkenfox_user_js_hash
```

To run only the non-snapshot integration tests:

```bash
cargo test --test mod
```

## Ignored Tests

Snapshot tests are automatically ignored when the `network-tests` feature is not enabled (they require network access to clone repositories). The test harness marks each file as ignored based on `cfg!(feature = "network-tests")`.
