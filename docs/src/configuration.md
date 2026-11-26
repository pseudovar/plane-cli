# Authentication and configuration

plane-cli needs three settings:

| Variable | Meaning |
|----------|---------|
| `PLANE_API_BASE_URL` | Your Plane URL, e.g. `https://plane.example.com` |
| `PLANE_API_KEY` | API key, sent as `X-API-Key` |
| `PLANE_WORKSPACE_SLUG` | The workspace part of your Plane URLs |

## Setup wizard

```bash
plane-cli config init            # writes ./.plane-cli.env
plane-cli config init --global   # writes the user-global file
plane-cli config update          # edits an existing file
plane-cli config where           # shows which files and keys are in use (never values)
plane-cli config doctor          # checks settings and API reachability
```

The wizard writes env files with mode `600`.

## Load order

The first layer that sets a key wins:

1. Non-empty process environment
2. `--env-file PATH`
3. `PLANE_DOTENV`: `1`/`true`/`yes` loads `./.env`, `0`/`false`/`no` is off, and any other value is a file path. Read from the process environment only
4. `--dotenv` (`./.env`)
5. `./.plane-cli.env`
6. User-global file: `$XDG_CONFIG_HOME/plane-cli/.plane-cli.env`, or `~/.config/plane-cli/.plane-cli.env`

The full list of variables is in [JSON output and exit codes](agent-contract.md#environment).

## Files in the current directory

A repository you clone can contain `.plane-cli.env` or `.env`, so plane-cli does not trust files in the current directory. Only the current directory is read; parent directories are not searched.

- Only `PLANE_API_BASE_URL`, `PLANE_API_KEY`, `PLANE_WORKSPACE_SLUG`, `PLANE_CACHE` and `PLANE_CLI_JSON` are read from these files. Everything else is ignored, including `PLANE_CLI_AS`, `PLANE_CLI_ANON_TELEMETRY`, `PLANE_DOTENV`, proxy variables, `HOME` and `PATH`. Ignored `PLANE_*` settings produce a warning.
- `PLANE_API_BASE_URL` from such a file is used only when **the same file also supplies the `PLANE_API_KEY` that will be sent**. Otherwise it is ignored with a warning, and the URL comes from the next layer. Your global or process key is never sent to a URL from the current directory.
- A file that sets any of those settings and contains `$` anywhere is a **hard error** (`env_file_unsafe`), because env-file parsing would expand `$VAR` and `${VAR}` from your environment. The command stops rather than falling back to another server. A file with `$` that sets none of them, such as another app's `.env`, is ignored with a warning. `config update` refuses to edit a file containing `$`.
- The file must be a regular file of at most 64 KiB. A symlink, FIFO or device is ignored with a warning. This includes a `~/.plane-cli.env` managed by a dotfile tool when you run from `~`; keep that setup in the global file, or pass it with `--env-file`. A file that is too large (`env_file_too_large`) or not valid (`env_file_invalid`) is an error, and the error never quotes its content.
- When a URL from such a file is used, plane-cli warns and names the host. **Treat that server's responses as untrusted content.** A repository can point plane-cli at a server it controls, and work item titles, descriptions and comments from it could contain prompt injection aimed at an AI agent.
- With `--as` or `PLANE_CLI_AS`, a URL from such a file is refused with `member_key_untrusted_url`, before the member key is read from the environment, Keychain or key file.

These rules also apply to `--dotenv`, and to `PLANE_DOTENV` when it points inside the current directory (symlinks and `..` are resolved). Your global env file stays trusted, and may be a symlink, including when the current directory is `~/.config/plane-cli`. A file named with `--env-file`, or an absolute `PLANE_DOTENV` path outside the current directory, is trusted because you chose it.

A file in the current directory can still set its own key or workspace slug. If you do not trust a repository, check its `.plane-cli.env` or run `plane-cli config where` first.

## Keeping keys out of env files

An env file is plain text. To keep your key in the macOS Keychain or a mode-600 key file:

```bash
plane-cli config set-key --member alice
```

Then set `PLANE_CLI_AS=alice` in the global env file and delete the `PLANE_API_KEY=` line. See [Agents and `--as`](agent-identity.md).

## Redirects and workspace slug

The API key is sent only to the configured origin. plane-cli follows a redirect only when the scheme, host and port are unchanged. Any other redirect, including `http` to `https`, is an error, so set `PLANE_API_BASE_URL` to the final URL. Attachment downloads use presigned storage URLs and do not carry the key.

`PLANE_WORKSPACE_SLUG` may contain only letters, digits, `-` and `_`.

## Cache

Catalogs such as states and labels are cached on disk. Pass `--refresh` to bypass the cache for one command, or set `PLANE_CACHE=0` to turn it off.
