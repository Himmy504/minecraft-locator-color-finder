# Troubleshooting

## The first scan downloads a very large file

The default profile snapshot is 1,649,493,087 bytes compressed. This is expected. It is stored in the operating system's application cache rather than the repository.

If the connection stops, run the command again. The downloader retains its `.part` file and requests only the remaining bytes when the server supports HTTP ranges.

## The integrity check fails

The pinned dataset must match both its expected byte length and SHA-1 digest. Retry from a clean partial download:

```shell
locator-color-finder download --force
```

Repeated failures usually indicate a proxy, filtering product, disk problem, or upstream download issue. Do not bypass the integrity check.

## A custom CSV is rejected

Custom input must have no header and exactly two fields per row:

```csv
03dd8f28bb344feaac00d8e925d3f3c1,Filiposz321
```

UUIDs may use compact or canonical dashed form. Usernames must contain 1-16 ASCII letters, digits, or underscores. The error reports the row where parsing stopped.

## Name verification reports failures

The matching UUID remains in the output with `name_status=verification_failed`. This can happen because of a network outage, timeout, proxy, Mojang rate limit, or upstream server failure.

Rerun later, or disable all Mojang verification requests:

```shell
locator-color-finder scan --no-verify-names
```

## The result count is not 74

The known count of 74 applies only to exact rendered color `#39E6E5` in the pinned `minecraft-uuids-2025-09-01` snapshot. Counts can differ when using another color, another dataset, an incomplete download, or a dataset with different profiles.

The program cannot discover accounts absent from the selected dataset because Mojang does not expose a complete account-enumeration API.

## The in-game marker has another color

The program calculates the UUID-derived default. Scoreboard teams, `/waypoint` commands, server plugins, mods, or later game changes can override it.

## Windows SmartScreen warns about the executable

GitHub release binaries are not code-signed. Verify the downloaded archive against `SHA256SUMS.txt` from the same release. If you do not want to run an unsigned binary, build from source with `cargo build --release --locked`.

## The output file cannot be replaced

Close the CSV in Excel or another program and retry. Windows applications commonly hold exclusive locks that prevent atomic replacement.

