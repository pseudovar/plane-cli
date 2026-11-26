# Telemetry

Error telemetry is **off by default**. plane-cli sends nothing unless both of these are true:

1. You opted in with `PLANE_CLI_ANON_TELEMETRY=1` (or `true`/`yes`) in the process environment, the global env file or a file you name with `--env-file`.
2. The binary was built with a telemetry endpoint. Official release binaries have one. A source build does not, so it never sends telemetry, even when opted in.

Check the state with:

```bash
plane-cli --json config doctor --offline
```

`data.anon_telemetry` reports `enabled` (you opted in), `build_has_endpoint`, and `active` (both).

## Turning it off

Unset `PLANE_CLI_ANON_TELEMETRY`, set it to `0`, or answer no in `plane-cli config update --global`. Check your env files too: `plane-cli config where` lists the keys each one sets. The setting is never read from a file in the current directory.

## What is sent

Reports go to [Sentry](https://sentry.io/). When telemetry is active:

| When | What |
|------|------|
| Every command | A performance record: the command path (such as `work-items list`), duration, success or error status, and whether JSON mode was on |
| Usage error (bad flags) | The error kind, the command path, the exit code, and the name of the offending flag. A stray value is sent as `<positional>` and an unknown subcommand as `<subcommand>` |
| HTTP error | The error code, status, method and exit code, and the API path with every slug, identifier and id replaced by `{id}`, such as `/api/v1/workspaces/{id}/projects/{id}/cycles/` |
| Network error | The error code, and the name of the config layer that set the URL (such as `global`). Never the host, URL or file path |
| Any other error | The error code and exit code only |
| TUI background failure | A fixed message, with no error text |
| Crash (panic) | The panic type, its source location, and a **stack trace**: function names, source file paths and line numbers from the machine that built the binary. The panic message is removed, because it could quote your data |

Each report also includes the plane-cli version, OS name and version, kernel version, CPU architecture, the Rust compiler version, and on macOS the hardware model.

## What is not sent

- API keys. Every event also passes a scrubber that replaces the key in use, any `PLANE_API_KEY` or `PLANE_API_KEY_<MEMBER>` value in the environment, and any `plane_api_…` token.
- Your Plane server's URL or hostname.
- Workspace, project, cycle, module, state, label or member names, emails, work item ids, or ids of any kind.
- Work item titles, descriptions or comments.
- Local file paths, env file contents, HTTP response bodies, or the server's error text.
- Your machine's hostname or username. plane-cli reports a fixed server name.

Sentry, like any web service, sees the IP address of the connection. plane-cli does not add it to events.

Write-policy refusals are expected outcomes and are not reported.

## Other network requests

Apart from your Plane server and telemetry, plane-cli contacts GitHub in these cases:

- `self check` and `self update`.
- The TUI checks for a newer release when it starts. Set `PLANE_CLI_NO_UPDATE_CHECK=1` to turn this off.

## Building with telemetry

The endpoint is read at compile time from `PLANE_CLI_SENTRY_DSN`. If you distribute your own builds, set it to your own Sentry project, or leave it unset to build without telemetry.
