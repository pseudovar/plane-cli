# Troubleshooting

Start with these two commands. Neither prints your API key.

```bash
plane-cli config doctor
plane-cli config where
```

| Symptom | Likely cause and fix |
|---------|----------------------|
| `network_error` | The URL is wrong or the server is down. The message names the host and the config layer that set it. A stale value in an env file is the usual cause |
| `authentication_failed` (401) | The API key is wrong or revoked. Run `plane-cli config update` |
| `permission_denied` (403) | The key's member lacks access to that workspace or project |
| `config doctor` says `not_plane` | Something answered at that URL, but it is not the Plane API. Check the base URL and any reverse proxy |
| Variables reported missing after `config init` | Run from the directory holding `.plane-cli.env`, use `config init --global`, or pass `--env-file` |
| Stale state or label names | Pass `--refresh`, or set `PLANE_CACHE=0` |
| HTTP 3xx: `Plane redirected to … a different origin` | The base URL is not the final address (often `http` instead of `https`, or a missing `www`). Set `PLANE_API_BASE_URL` to the address named in the error |
| `env_file_unsafe`, `env_file_too_large`, `env_file_invalid` | A file in the current directory was refused and nothing was sent. Fix or remove the named file, or pass a file you trust with `--env-file` |
| A warning that a file `was ignored` | A file in the current directory is a symlink or other non-regular file, or has `$` and no plane-cli settings, or has a setting it may not set. See [Files in the current directory](configuration.md#files-in-the-current-directory) |
| `project_mismatch` | `--project` differs from the prefix in `--work-item`. Drop `--project` or make them match |
| `project_required_for_uuid` | Use a `ENG-123` style id, or add `--project` |
| `body_invalid_json` | `--body` takes a file path or `-` for stdin, not inline JSON |
| `policy_violation` | The project's [write policy](policy.md) refused the write. `error.details.violations` lists each fix |
| `member_key_not_found` | No key is stored for the `--as` member. See [Agents and `--as`](agent-identity.md) |
| `member_identity_mismatch` | The key belongs to a different Plane user than the member name |
| `self check` fails with a rate limit | GitHub limits unauthenticated requests per IP address. Try again later |
| Install script reports a checksum error | The download was corrupted or incomplete. Retry, and report it if it persists |

For a bug report, include `plane-cli --version` and your Plane version. Remove API keys and private hostnames from anything you paste.
