# Install

## Prebuilt binaries

Releases include Linux x86_64 (glibc), macOS arm64 and macOS x86_64 binaries. The install script needs `curl` or `wget`, `tar`, and `sha256sum` or `shasum`. It verifies the SHA-256 checksum and refuses to install if the checksum is missing or wrong. Downloads must stay on HTTPS: curl enforces this for every redirect, and with wget the script checks the redirects afterwards and discards the file if one left HTTPS. Prefer curl when both are available.

```bash
curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh | bash
```

To read the script first:

```bash
curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh -o install.sh
less install.sh
bash install.sh
```

Options:

| Option | Effect |
|--------|--------|
| `bash install.sh v0.3.0` or `PLANE_CLI_VERSION=v0.3.0` | Install that version. Through a pipe: `… \| bash -s -- v0.3.0` |
| `PREFIX=/opt/plane` | Install to `$PREFIX/bin` (default `$HOME/.local`) |
| `PLANE_CLI_PRERELEASE=1` | Include prereleases when resolving the latest version |
| `PLANE_CLI_FORCE=1` | Reinstall even if that version is already at the install location |

## From source

Requires Rust 1.86 or newer.

```bash
git clone https://github.com/pseudovar/plane-cli
cd plane-cli
cargo build --release -p plane-cli
# binary: target/release/plane-cli
```

A source build has no telemetry endpoint, so it never sends telemetry. See [Telemetry](telemetry.md).

## Updating

```bash
plane-cli self check          # compare this binary with the latest stable release
plane-cli self update         # print the install command
plane-cli self update --apply # replace this binary with the latest release
```

`--apply` only moves forward to a newer version. It updates the running binary's own location, which must be `<prefix>/bin/plane-cli` and writable; otherwise it stops without changing anything. It then checks that the binary reports the new version.

These commands use public GitHub URLs and do not need the GitHub CLI or a login. GitHub limits unauthenticated API requests per IP address, so a check can fail briefly on a shared network.

## Shell completions

```bash
plane-cli completions --install
```

See [Shell completions](agent-contract.md#shell-completions) for the options.

## Agent skill

The repository includes a skill for AI coding agents. Install it from the binary, with no other tools:

```bash
plane-cli self skills install --local      # ~/.agents, ~/.claude, ~/.cursor and Codex skill folders
plane-cli self skills install --openclaw   # for OpenClaw-compatible runners
plane-cli self skills check
```

`plane-cli self skills install` with no flag runs `npx skills@latest add pseudovar/plane-cli --skill plane-cli`, which needs Node.js.
