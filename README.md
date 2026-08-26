# Minecraft Java Locator-Bar Account Finder

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)

`locator-color-finder` is a fast, cross-platform command-line program that scans Minecraft Java UUIDs and finds every profile in a selected dataset whose **default rendered Locator Bar color** matches a requested RGB value.

The default target is `#39E6E5`. The pinned public snapshot contains 65,936,987 profiles and has 74 matching UUIDs.

> [!IMPORTANT]
> “Every profile” means every match in the selected dataset. Mojang does not provide a public API that enumerates every Minecraft account, so no program can truthfully guarantee a complete global account list.

## Features

- Reproduces Minecraft Java's UUID-derived color calculation with Java-compatible `f32` rounding.
- Searches any rendered RGB color; `#39E6E5` is simply the default.
- Streams `.csv` and `.csv.zst` inputs without extracting them or creating a database.
- Downloads and resumes the pinned 1.65 GB profile snapshot automatically.
- Checks the pinned dataset's exact length and SHA-1 digest before scanning.
- Optionally refreshes only matched usernames through Mojang's public profile endpoint.
- Writes deterministic, atomic CSV output with verification and dataset provenance.
- Runs on Windows, Linux, and macOS.

## Quick start

### Use a release binary

Download the archive for your operating system from the [latest GitHub release](../../releases/latest), extract it, and run:

Windows PowerShell:

```powershell
.\locator-color-finder.exe scan
```

Linux or macOS:

```shell
chmod +x ./locator-color-finder
./locator-color-finder scan
```

The first default scan downloads approximately 1.65 GB. The verified compressed dataset is kept in the operating system's application cache, not in the project directory.

### Build from source

Install [Rust](https://www.rust-lang.org/tools/install) 1.88 or newer, open a terminal in the repository, and run:

```shell
cargo build --release --locked
cargo run --release --locked -- scan
```

The compiled binary is created under `target/release/` (`locator-color-finder.exe` on Windows).

## Commands

Show built-in help:

```shell
locator-color-finder --help
locator-color-finder scan --help
```

### Scan the default snapshot

```shell
locator-color-finder scan
```

This searches for the default rendered color `#39E6E5`, verifies current names, and creates `matches.csv` in the current directory.

### Search another rendered color

```shell
locator-color-finder scan --color "#E66185" --output notch-color.csv
```

Colors accept exactly six hexadecimal digits, with or without `#`.

### Search your own dataset

```shell
locator-color-finder scan --dataset /path/to/players.csv.zst --output results.csv
```

On Windows:

```powershell
.\locator-color-finder.exe scan --dataset "D:\data\players.csv.zst"
```

Custom input can be plain CSV or Zstandard-compressed CSV. It must have no header and exactly one `uuid,username` pair per line:

```csv
03dd8f28bb344feaac00d8e925d3f3c1,Filiposz321
03fb97018e8242579845d41e5302f8b0,Ryxn___
```

UUIDs may be compact or canonically dashed. Usernames must contain 1-16 ASCII letters, digits, or underscores. Malformed input stops the scan with the affected row number so an “all matches” result is never silently incomplete.

### Work offline

After the dataset exists locally, prevent Mojang username lookups with:

```shell
locator-color-finder scan --no-verify-names
```

### Download the snapshot separately

```shell
locator-color-finder download
```

Restart from a clean download if the cached copy is damaged:

```shell
locator-color-finder download --force
```

Interrupted downloads retain a `.part` file and resume on the next run when the server supports HTTP ranges.

## Result format

The output CSV contains:

| Column | Meaning |
| --- | --- |
| `uuid` | Canonical dashed Java UUID. |
| `snapshot_username` | Username stored in the selected dataset. |
| `current_username` | Mojang's current username when verification succeeds. |
| `name_status` | `unchanged`, `renamed`, `not_found`, `verification_failed`, or `not_requested`. |
| `internal_color` | Low 24 bits of the UUID hash before brightness normalization. |
| `rendered_color` | Final default Locator Bar color. |
| `dataset_id` | Pinned snapshot name or `custom:<filename>`. |
| `verified_at_utc` | Mojang verification time, blank when verification was disabled. |

Only matched UUIDs are sent to Mojang. Verification uses four concurrent requests, 10-second timeouts, and up to three retries for transport failures, rate limits, and transient server errors. A failed verification never removes a dataset match.

Exit codes are stable for scripting:

| Code | Meaning |
| --- | --- |
| `0` | Scan or download succeeded, including a scan with zero matches. |
| `1` | Runtime, network, parsing, output, or integrity failure. |
| `2` | Invalid command-line arguments. |

## Why the algorithm is unusual

Minecraft Java computes the default color through this client path:

```text
hash     = uuid.hashCode()
internal = hash & 0xFFFFFF
rendered = ARGB.setBrightness(ARGB.color(255, hash), 0.9F)
```

Java's UUID hash is the XOR of the UUID's four big-endian 32-bit segments. `setBrightness` preserves HSV-like hue and saturation while replacing brightness/value with `0.9F`, then converts back using Java `float` arithmetic and `Math.round` behavior.

It does **not** multiply each RGB channel by 0.9. That widespread approximation produces incorrect colors. For example:

| UUID | UUID hash | Internal | Correct rendered color |
| --- | --- | --- | --- |
| `069a79f4-44e9-4726-a5be-fca90e38aaf5` | `E9F5688E` | `#F5688E` | `#E66185` |
| `03dd8f28-bb34-4fea-ac00-d8e925d3f3c1` | `313AEBEA` | `#3AEBEA` | `#39E6E5` |

Twenty-six different internal RGB values render as `#39E6E5`. The program computes the exact inverse set before scanning, then performs a constant-time hash lookup for every dataset row.

## Accuracy and scope

- Supported calculation: Minecraft Java Edition's default UUID-derived color from 1.21.6 through 26.2.
- Excluded: Bedrock/XUID profiles and offline-mode name-derived UUIDs.
- Scoreboard team colors, `/waypoint` overrides, plugins, mods, resource packs, and future game changes can alter what appears in-game.
- Username verification refreshes known matches but cannot discover UUIDs absent from the selected dataset.
- A newer compatible dataset can be supplied at any time through `--dataset`.

## Development

Normal development and CI use only small local fixtures; the 1.65 GB dataset is not required.

```shell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

The full pinned-corpus acceptance test is available but ignored by default:

```shell
cargo run --release --locked -- download
cargo test --release --locked pinned_full_snapshot_has_74_matches -- --ignored --nocapture
```

See [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. New maintainers can follow [the GitHub publishing guide](docs/PUBLISHING.md), then use [the release guide](docs/RELEASING.md). Common operational problems are covered in [the troubleshooting guide](docs/TROUBLESHOOTING.md). Security issues should follow [SECURITY.md](SECURITY.md).

## Dataset and research sources

- [Minecraft Java Edition 1.21.6 release notes](https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-6)
- [Minecraft Java Edition 26.2 release notes](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-2)
- [Java `UUID` documentation](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/UUID.html)
- [65,936,987-profile dataset, published 2025-09-01](https://archive.org/details/minecraft-uuids-2025-09-01)
- [Independent hosted reference query for `#39E6E5`](https://locatorbar.crab.trade/api/searchrendered?color=39e6e5)

The pinned `players.csv.zst` is 1,649,493,087 bytes with SHA-1 `26b25dbe6728bf6398e8ba4d5eaaed335ab0cea4`. The dataset is downloaded with attribution and is not redistributed in this repository.

## License and disclaimer

The program is available under the [MIT License](LICENSE).

This is an independent, unofficial utility. It is not affiliated with, sponsored by, or endorsed by Mojang Studios or Microsoft. Minecraft is a trademark of Microsoft Corporation.
