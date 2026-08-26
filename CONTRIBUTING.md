# Contributing

Thank you for improving Locator Color Finder. Contributions should remain focused on Minecraft Java's UUID-derived default Locator Bar color, reproducible dataset scanning, or the CLI workflow around those features.

## Development setup

Install [Rust](https://www.rust-lang.org/tools/install) 1.88 or newer with the `rustfmt` and `clippy` components:

```shell
rustup component add rustfmt clippy
cargo build --locked
```

No account, API token, database, or downloaded player corpus is required for normal development. Automated tests use miniature local fixtures and local HTTP servers.

## Required checks

Run these before opening a pull request:

```shell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

The full 65.9-million-row acceptance test is intentionally ignored during ordinary CI. To run it:

```shell
cargo run --release --locked -- download
cargo test --release --locked pinned_full_snapshot_has_74_matches -- --ignored --nocapture
```

## Pull requests

- Keep changes focused and explain observable behavior.
- Add regression tests for bug fixes and tests for new behavior.
- Preserve exact `f32` operations in the Minecraft color implementation. Algebraically similar `f64` code can disagree at rounding boundaries.
- Keep scanning streaming and bounded-memory; do not require an extracted dataset or database.
- Do not commit downloaded datasets, generated match lists, access tokens, or player data unrelated to a test fixture.
- Update the README, troubleshooting guide, or changelog when users or maintainers need to know about a change.

## Dataset fixtures

Test fixtures must be minimal and contain only public Minecraft UUID/profile data needed for a deterministic test. Prefer synthetic data where it can exercise the same behavior. Never contribute private server logs or privately collected player lists.

## Commit and release notes

Use concise imperative commit subjects, such as `Handle servers that ignore Range requests`. User-visible changes should be recorded under `Unreleased` in `CHANGELOG.md`.

Maintainers should follow [the release guide](docs/RELEASING.md).

