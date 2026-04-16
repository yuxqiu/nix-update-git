# Contributing Tests with Insta

This project uses [insta](https://insta.rs/) for snapshot testing. Tests are defined in `tests/snapshot/`.

## Writing a New Test

1. **Add a Nix file** in `tests/snapshot/data/<category>/<test_name>.nix`

2. **Run the test** to generate the snapshot:

   ```bash
   cargo test --features network-tests test_fetcher_snapshots -- -s
   ```

   The `-s` flag updates snapshots in review mode (opens your editor for each snapshot).

   Alternatively, use `-u` to update all snapshots unconditionally:

   ```bash
   cargo test --features network-tests test_fetcher_snapshots -- -u
   ```

3. **Verify the snapshot** was created at `tests/snapshot/snaps/<category>/<test_name>.snap`

## Test Structure

Nix files in `data/` are processed by `test_fetcher_snapshots()` in `tests/snapshot/mod.rs`:

- Each `.nix` file is run through `nix-update-git --format json`
- JSON output is parsed into `SnapshotEntry` structs (rule, field, old)
- The result is snapshot under `snaps/<category>/<test_name>.snap`

## Updating Existing Snapshots

```bash
cargo test --features network-tests test_fetcher_snapshots -- -u
```

Use `-s` for review mode to inspect each change individually.

## Running Tests

```bash
cargo test                     # skip network tests (default)
cargo test --features network-tests  # include network tests
```

## Redacting Sensitive Values

Edit the `redaction()` function in `tests/snapshot/mod.rs` to add regex patterns for values that should be masked in snapshots.
