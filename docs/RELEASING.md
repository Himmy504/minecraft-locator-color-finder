# Release guide

This guide is for project maintainers.

## 1. Prepare the release

1. Move completed entries from `Unreleased` into a new version section in `CHANGELOG.md`.
2. Update `version` in `Cargo.toml` and run `cargo check --locked` so `Cargo.lock` records the package version.
3. Run the required checks:

```shell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

4. Commit the release changes to the default branch.

## 2. Tag the release

The annotated tag must exactly match the `Cargo.toml` version with a leading `v`:

```shell
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

## 3. Automated publication

The GitHub Release workflow validates the version, formatting, Clippy, and tests before building:

- Windows x86-64 ZIP
- Linux x86-64 tarball
- macOS x86-64 tarball
- macOS ARM64 tarball
- `SHA256SUMS.txt`

It then creates a GitHub release with generated notes. If validation fails, delete the local tag, fix the source on the default branch, and create a new tag only after the fixes are merged. Do not move or replace an already published release tag.

## 4. Verify the release

Download one archive and its checksum, verify it, and run:

```shell
locator-color-finder --version
locator-color-finder scan --dataset tests/fixtures/players.csv --no-verify-names
```

Confirm that the miniature fixture reports two matches for `#39E6E5`.
