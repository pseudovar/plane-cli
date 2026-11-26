# Security policy

## Reporting a vulnerability

Please report security problems privately through [GitHub private vulnerability reporting](https://github.com/pseudovar/plane-cli/security/advisories/new). Do not open a public issue or discussion.

Include the affected version, steps to reproduce and the impact you expect. Remove real API keys and hostnames from your report. This is a volunteer project, so response times vary, but reports are treated as a priority.

Vulnerabilities in Plane itself should go to the Plane project.

## Supported versions

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Older releases | No. Please update with `plane-cli self update --apply` |

## How credentials are stored

plane-cli stores API keys only on your machine and sends them only to the Plane server you configure, in the `X-API-Key` header.

| Location | Protection |
|----------|------------|
| Process environment (`PLANE_API_KEY`, `PLANE_API_KEY_<MEMBER>`) | Managed by you or your secret store |
| Env file from `config init` (`.plane-cli.env`) | Plain text, written with mode `600`. `config doctor` warns about a key stored this way |
| macOS Keychain (`config set-key`) | Keychain item `plane-cli-<member>` |
| Key file on Linux and other Unix (`config set-key`) | `~/.config/plane-cli/keys/<member>`, mode `600` in a mode `700` directory. plane-cli refuses a file or directory with looser permissions, a symlink, or another owner |

`PLANE_API_KEY_<MEMBER>` is read only from the process environment, never from an env file. Error output and telemetry replace known keys and `plane_api_…` tokens with `<redacted>`. Identity checks cache a truncated SHA-256 fingerprint of a key, never the key.

## Files in the current directory

A cloned repository can contain `.plane-cli.env` or `.env`, so plane-cli restricts files in the current directory:

- Only the URL, API key, workspace slug, cache and JSON settings are read from them. The acting member, telemetry, `PLANE_DOTENV`, proxy variables and all other settings are ignored.
- A URL from such a file is used only with an API key from that same file. Your global, process or member key is never sent to it.
- With `--as`, a URL from such a file is refused before the member key is read.
- A file that sets a plane-cli setting and contains `$` is a hard error, so it cannot expand your environment variables into a value or fall back to another server.
- Only regular files of at most 64 KiB are read, opened without following symlinks. Errors never quote file content.
- When such a URL is used, plane-cli warns and names the host. Treat that server's responses as untrusted, including as possible prompt injection for AI agents.

The API key follows only same-origin redirects (scheme, host and port). Any other redirect is an error.

Telemetry is off by default. See the [telemetry docs](https://pseudovar.github.io/plane-cli/telemetry.html).
