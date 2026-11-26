# JSON output and exit codes

This document is the stable reference for **non-interactive** / **AI agent** use. Human-oriented text goes to **stderr**; **stdout** is reserved for JSON when `--json` is set (or `PLANE_CLI_JSON=1`, unless `--json` is passed on the CLI).

The **full-screen TUI** (`plane-cli` interactive mode) is documented in [Terminal UI](tui.md) — behavior, keybindings, and differences from `--json` automation (e.g. assignee picker scope).

## Environment

| Variable | Required | Meaning |
|----------|----------|---------|
| `PLANE_API_BASE_URL` | yes | Origin of the API, e.g. `https://plane.example.com` (no trailing slash required). |
| `PLANE_API_KEY` | yes, unless acting as a member | Sent as `X-API-Key`. Not read when `--as` / `PLANE_CLI_AS` picks a member (see **Acting as a member** below). |
| `PLANE_WORKSPACE_SLUG` | yes | Workspace segment in `/api/v1/workspaces/{slug}/...`. |
| `PLANE_CLI_AS` | no | Member to act as when `--as` is not given. |
| `PLANE_API_KEY_<MEMBER>` | no | A member's key for `--as <member>` (member upper-cased, `-` → `_`). Read **only from the process environment**; a value in an env file is ignored with a warning. |
| `PLANE_CLI_NO_UPDATE_CHECK` | no | If `1`/`true`/`yes`, the TUI does not check GitHub for a newer release at startup. |
| `PLANE_CLI_JSON` | no | If `1`/`true`/`yes`, same as `--json` (CLI `--json` wins if both set). |
| `PLANE_CACHE` | no | If `0` or `false`, disk cache reads/writes are disabled. |
| `PLANE_DOTENV` | no | `1`/`true`/`yes` loads `./.env`; `0`/`false`/`no` is off; any other value is a file path that must exist. Read from the process environment only. |
| `PLANE_CLI_ANON_TELEMETRY` | no | If `1`/`true`/`yes`, send error telemetry to Sentry (see [Telemetry](telemetry.md)). Default off. Source builds have no endpoint and send nothing. |

`plane-cli self check` and `plane-cli self update` do **not** use `PLANE_API_*` or workspace settings. They use the public GitHub API (no `gh` or login) and compare the running binary to **stable** GitHub releases (same rules as `scripts/install.sh`; prereleases are ignored unless you use the install script with `PLANE_CLI_PRERELEASE=1`).

Optional file loading:

- `--env-file PATH` — load that file first (dotenv semantics; does not override variables already non-empty in the process environment).
- `--dotenv` — load `.env` from the current directory.
- Precedence, highest first: non-empty process env → `--env-file` → `PLANE_DOTENV` → `--dotenv` → `./.plane-cli.env` → user-global file. The first layer that sets a key wins.
- Then **default plane-cli env files** apply for keys that are still unset or empty after the above: first the user-global `$XDG_CONFIG_HOME/plane-cli/.plane-cli.env` (fallback `~/.config/plane-cli/.plane-cli.env`; Windows `%APPDATA%\plane-cli\.plane-cli.env`), then `./.plane-cli.env`. Keys that were **non-empty** after `--env-file` / `PLANE_DOTENV` / `--dotenv` are never changed by these defaults. Between global and local defaults, **local wins** on conflicts.

Files in the current directory are restricted: see [Files in the current directory](configuration.md#files-in-the-current-directory).

`config doctor` runs the same loading sequence as other commands, then reports whether `PLANE_*` values are present and whether **`PLANE_CLI_ANON_TELEMETRY`** is enabled. When the variables are set it also makes one `GET /api/v1/users/me/` call with a 5 s timeout and reports `data.reachability.status`:

| `status` | Meaning |
|----------|---------|
| `reachable` | Plane answered with the user object. |
| `unreachable` | No answer; `reason` carries the OS error (e.g. `Connection refused (os error 111)`). |
| `unauthorized` | HTTP 401/403: the host is Plane but `PLANE_API_KEY` was rejected. |
| `not_plane` | Something answered (`http_status`), but not Plane's API. |
| `skipped` | `--offline`, or required variables are missing. |

`data.reachability` also carries `api_host` and `base_url_source` (`layer` + `path`, as below). `data.ok` is `false` unless the variables are set and the status is `reachable` (or `skipped` via `--offline`). The envelope's own `ok` stays `true`: the doctor ran. Checked against CE 1.4.2.

`config where` lists config **file paths** (global, local, and optional `--env-file` / `PLANE_DOTENV` / `--dotenv` layers for this invocation), whether each file exists, and **key names** defined in each file (never values). `data.base_url_source` names the layer that supplied the effective `PLANE_API_BASE_URL`: `process_env`, `cli_env_file`, `plane_dotenv`, `dotenv_cwd`, `global`, `local` (with `path` for files), `unset`, or `unknown`.

`config update` is an interactive wizard like `config init`, but reads an **existing** env file and shows each setting with its current value as the default (press Enter to keep). Supports `--global` and `--env-file PATH`. The API key is never shown; press Enter to keep it unchanged.

`config doctor` also reports `data.api_key_source` (`layer` + `path`, same names as `base_url_source`) and adds a **warning** when `PLANE_API_KEY` comes from a plain-text env file. With `--as` / `PLANE_CLI_AS` it reports `data.identity`: `member`, `origin` (`flag`, `process_env`, `env_file`), `key_found`, `key_source` (`kind` = `process_env` / `keychain` / `key_file`, plus `var` / `service` / `path`), and after a reachable probe `verified` and `user_id`, or `error` (`code`, `message`). The probe uses the member's key. `data.ok` is `false` when the member's key is missing or fails the check.

## Acting as a member (`--as`)

`--as <member>` (or `PLANE_CLI_AS`) makes the call with that Plane member's own API key, so Plane's activity log and comment authors show the member. Without it, nothing changes: `PLANE_API_KEY` is used. Member names are 1-32 characters of `a-z`, `0-9` and `-`, starting with a letter or digit (case is folded). `_` is refused so `ci-bot-2` and `ci_bot_2` cannot both map to `PLANE_API_KEY_CI_BOT_2`; `-` becomes `_` in the variable name.

Key lookup, first hit wins (`PLANE_API_KEY` is not used):

1. `PLANE_API_KEY_<MEMBER>` in the **process environment** (for example, set by your agent runner or CI). Never from an env file.
2. macOS: Keychain item `plane-cli-<member>` (`security find-generic-password -s plane-cli-<member> -w`).
3. Other Unix: `$XDG_CONFIG_HOME/plane-cli/keys/<member>` (fallback `~/.config/plane-cli/keys/<member>`). Refused unless it is a regular file (not a symlink; opened non-blocking, so a FIFO cannot hang the CLI) owned by the current user with no group or other permission bits (mode `600`; `400` also passes). The `keys` directory must itself be a real directory (not a symlink) owned by the current user and not writable by group or others (`member_key_dir_insecure`); `config set-key` creates it with mode `700`.

The key is then checked with `GET /api/v1/users/me/`: the user's `display_name` must equal the member name (ignoring case). Only when `display_name` is empty does the local part of `email` count. A mismatch is refused (`member_identity_mismatch`). A successful check is cached for 24 hours per key fingerprint (a truncated SHA-256; the key itself is never written), in a cache directory separate from other members and from the default identity; `--refresh` re-checks.

Trust on first use: the first Plane user id seen for a member (per instance and workspace) is pinned. A later check that returns a different user id for that member is refused with `member_identity_changed`, even with `--refresh`. `config set-key --member <member>` resets the pin (`data.identity_pin_reset`), so storing a key is the way to accept a replaced account. Pins live in the cache directory, so they need the cache enabled (`PLANE_CACHE` not `0`).

Where `PLANE_CLI_AS` may come from, in order:

1. An explicit `--as` always wins.
2. `PLANE_CLI_AS` in the process environment is used.
3. `PLANE_CLI_AS` in the per-directory `./.plane-cli.env` is **ignored** with a warning: a checked-out repository must not pick an identity. (`PLANE_API_KEY_<MEMBER>` is ignored from every env file, as above.)
4. `PLANE_CLI_AS` from another env file (global, `--env-file`, `PLANE_DOTENV`, `--dotenv`) gives way to a `PLANE_API_KEY` set in the process environment (CI override), with a warning. This is checked before the name is validated, so a bad value in a file cannot break such a run.

`invalid_member_name` messages name the source (`--as`, `--member`, `PLANE_CLI_AS (process env)`, `PLANE_CLI_AS (from global file …)`) but never the value.

`config set-key --member <member>` stores a key without echoing it: from a hidden TTY prompt, or from stdin when stdin is not a terminal. macOS writes the Keychain item through `security -i` (the key is never on a command line) and reads it back to confirm; other Unix writes the key file with mode `600` in a `700` directory. `data.stored_in` has the same shape as `key_source`. Member onboarding steps: [Agents and `--as`](agent-identity.md). `config init` and `config update` read `PLANE_API_KEY` with input hidden and never show the current value (`[keep current]`).

Keys are kept out of output: errors name the source (variable name, Keychain item, file path), not the key. As a backstop, every error envelope (message, `details`, `warnings`) and human error line has the key that was sent, any `PLANE_API_KEY` / `PLANE_API_KEY_<MEMBER>` value in the environment, and any `plane_api_…` token replaced by `<redacted>`. Clap usage errors show a stray positional argument as `<positional>`, since it could be a pasted key. Success output (`data`) is Plane's response and is not rewritten.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `2` | Usage / local validation: bad flags, missing required args, resolve errors (unknown project/state name), corrupt local JSON, strict catalog mismatch **before** a successful HTTP response. |
| `1` | HTTP or transport failure after a request is attempted, I/O, cache, JSON parse errors on responses, or other internal errors. |

## Success JSON (`--json`)

Single JSON object on stdout:

```json
{
  "ok": true,
  "data": { },
  "next_cursor": "optional cursor string",
  "truncated": true
}
```

- `next_cursor` / `truncated` appear only for some list operations (when applicable).
- `data` is the decoded API response body (object, array, or `null` for empty deletes).
- `warnings` (array of strings) appears when the CLI accepted a tolerated mistake and corrected it; fix the call before the next one. Current cases: positional ids (`comments list ENG-123`), inline JSON in `--body`, hidden flag aliases (`--comment-id` / `--link-id` for `--id`, `--work-item` for `--work-items` on `modules issues add|remove` and `cycles items add|remove`), and the `issue` / `issues` / `ticket` / `work-item` commands, which are forwarded to `work-items` when the rest of the call parses (otherwise they return a usage error with guidance).

## Error JSON (`--json`)

```json
{
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "human-readable summary",
    "http": {
      "status": 404,
      "method": "GET",
      "path": "/api/v1/workspaces/..."
    },
    "rate_limit": {
      "remaining": 45,
      "reset_epoch": 1700327957
    },
    "details": { }
  }
}
```

- `http` is present for failed HTTP responses.
- `rate_limit` is present when the server sent `X-RateLimit-*` headers (often on `429`).
- `details` is optional:
  - for failed HTTP responses, may contain the API error body (never includes your API key)
  - for local usage/config/resolve errors, may contain corrective hints such as `hint` and `suggested_command`

### Stable `error.code` values

| `code` | Typical cause |
|--------|----------------|
| `validation_error` | Local validation, HTTP 400, or usage/config/resolve (see exit code `2` vs `1`). |
| `project_mismatch` | `--project` names a different project than the `PREFIX-N` `--work-item`. Exit `2`; nothing is changed. |
| `project_required_for_uuid` | `--work-item` is a UUID and `--project` is missing. A `PREFIX-N` id works without `--project`. Exit `2`. |
| `body_file_not_found` | `--body PATH` names a file that does not exist; the message contains the path. Exit `2`. |
| `body_file_unreadable` | `--body PATH` is a directory or cannot be read. Exit `2`. |
| `body_invalid_json` | `--body` content (file, stdin, or inline) is not valid JSON. Exit `2`. |
| `invalid_member_name` | `--as` / `PLANE_CLI_AS` / `--member` is not a valid member name, or looks like a pasted API key. The value is not echoed. Exit `2`. |
| `member_key_not_found` | No key for the member in the process env, Keychain or key file (the message lists what was checked), or the key file is empty. Exit `2`. |
| `member_key_file_insecure` | The key file is a symlink, not a regular file, owned by someone else, or has group/other permission bits. Exit `2`; run `chmod 600 <path>`. |
| `member_key_file_unreadable` | The key file exists but cannot be opened or read. Exit `2`. |
| `member_identity_mismatch` | `users/me` says the member's key belongs to another Plane user; nothing was done. Exit `2`. |
| `member_identity_changed` | The member's key belongs to a different Plane user id than the one pinned on first use; nothing was done. Exit `2`; `config set-key --member <member>` resets the pin. |
| `member_key_dir_insecure` | The `keys` directory is a symlink, not a directory, owned by someone else, or writable by group/other. Exit `2`; run `chmod 700 <dir>`. |
| `member_key_rejected` | Plane answered 401/403 to the member's key (revoked or mistyped). Exit `2`. |
| `member_key_untrusted_url` | `PLANE_API_BASE_URL` comes from a file in the current directory, so the member's key is not read or sent. Exit `2`; set the URL in the global file, the process environment or `--env-file`. |
| `env_file_unsafe` | A file in the current directory sets a plane-cli setting and contains `$`, or a required one is not a regular file. Nothing is sent. Exit `2`. |
| `env_file_too_large` | A file in the current directory is over 64 KiB. Exit `2`. |
| `env_file_invalid` | A file in the current directory cannot be parsed; its content is not quoted. Exit `2`. |
| `env_file_unreadable` | A file in the current directory cannot be read. Exit `2`. |
| `keychain_error` | macOS `security` failed to read or write the member's Keychain item. Exit `2`. |
| `invalid_api_key` | `config set-key` input is not 8-512 printable ASCII characters without spaces or quotes; nothing was stored. Exit `2`. |
| `member_key_store_unsupported` | `config set-key` on a platform without a supported store (use `PLANE_API_KEY_<MEMBER>`). Exit `2`. |
| `handlers_file_unreadable` | `inbox --handlers PATH` cannot be read. Exit `2`. |
| `handlers_invalid_json` | `inbox --handlers` is not JSON or has no `projects` object. Exit `2`. |
| `policy_violation` | The project's write policy refused this write; `details.rule`, `details.fix`, `details.violations`, `details.policy_file`. Nothing was written. Exit `2`. |
| `policy_invalid` | The project's policy file does not parse or has an unknown rule or field; every write to that project fails until it is fixed. Exit `2`. |
| `policy_unknown_state` | A policy with state rules could not find the requested state in the project, even after refreshing the states catalog. Nothing was written. Exit `2`. |
| `policy_exists` | `config policy init` without `--overwrite` and the file exists. Exit `2`. |
| `invalid_project_identifier` | `config policy` `--project` is not a 1-12 character identifier (UUIDs are not accepted). Exit `2`. |
| `authentication_failed` | HTTP 401. |
| `permission_denied` | HTTP 403. |
| `not_found` | HTTP 404. |
| `conflict` | HTTP 409. |
| `rate_limited` | HTTP 429. |
| `server_error` | HTTP 5xx. |
| `http_error` | Other HTTP status. |
| `network_error` | Connection / TLS / timeout. The message names the host tried and the config layer that set `PLANE_API_BASE_URL`; `details` has `api_host`, `config_layer`, `config_path`, `reason`. A stale URL in an env file is the usual cause: check `config where`, then `config doctor`. |
| `json_error` | serde JSON error. |
| `url_error` | URL parse error. |
| `io_error` | Local I/O. |
| `cache_error` | Cache read/write. |
| `unexpected_response` | Plane answered 2xx but the body lacked a field the command needs (exit `1`). |

When a multi-step command fails part-way (`cycles items remove`, `modules issues remove`, `work-items attachments create`), `error.code` and the exit code are those of the step that failed, and `error.details` describes what already happened (for removals `{ "removed": [...], "failed": "<input>" }`; for uploads `{ "step", "asset_id", "cleaned_up" }`). Plane's own error body, when there is one, is under `error.details.cause`.

## Strict catalog (`--strict-catalog`)

When set, **before** `work-items create` / `work-items update`, the CLI:

1. Loads **states** and **labels** for the resolved project (disk cache + `--refresh` applies). `labels create|update|delete` and `states create|update|delete` drop the cached catalog for that project, so the next lookup sees the change without `--refresh`.
2. Normalizes **`state`**: UUID must exist in the catalog; otherwise treated as a **state name** (case-insensitive).
3. Normalizes **`labels`**: each entry must be a label UUID in the catalog or a **label name** (case-insensitive).
4. Normalizes **`assignees`**: must be a JSON array of **user UUID** strings (no name resolution in strict mode).

The API still remains authoritative: a passing strict check does not guarantee the server accepts the payload.

## Work items (`work-items create` / `work-items update`)

Non-interactive parity with the TUI PATCH fields:

- **`--state`**, **`--priority`**, **`--name`**, **`--description`** — merged into the request body (with `--strict-catalog` resolving state/labels where applicable). For work items, `--description` is sent as Plane's `description_html` field.
- **`--body PATH`** — JSON request body from a **file path** or **`-`** (stdin). Not an inline JSON string. Example: `echo '{"assignees":[]}' | plane-cli … --body -`.
- **`--assignee UUID`** — repeatable; builds `"assignees": ["<uuid>", …]`. Omit the flag to leave assignees unchanged on update. To **clear** all assignees, pass JSON via `--body -` with `{"assignees":[]}` (merged with other flags per the command’s merge rules).

Common agent-safe update patterns:

```bash
# Description only
plane-cli --json work-items update --project ENG --work-item ENG-123 \
  --description "Knowledge resource: https://example.com/doc"

# Description + labels
echo '{"labels":["bug","tech-debt"]}' | plane-cli --json --strict-catalog work-items update \
  --project ENG --work-item ENG-123 \
  --description "Knowledge resource: https://example.com/doc" \
  --body -
```

The interactive TUI uses the same PATCH semantics but only offers assignees already visible on **loaded work-item pages** in its picker; use **`--assignee`** for arbitrary user UUIDs.

## Project preflight (`projects context`, enriched `complete`)

Before mutating work items in an unfamiliar project, prefer a single snapshot:

```bash
plane-cli --json projects context --id <PROJECT>
```

`data` includes `identity`, `capabilities` (feature flags from the project record, plus `archived`, true when the project is archived), `states` grouped by workflow bucket (`backlog`, `unstarted`, `started`, `completed`, `cancelled`, `triage`), `states_flat`, `labels`, optional `modules` summaries when `module_view` is enabled, optional `cycles` summaries (`id`, `name`, `start_date`, `end_date`, `status`, `archived`) when `cycle_view` is enabled, `summary.counts` (work items, cycles, modules, members, states, labels, intakes, pages from `projects summary`; `null` with a warning if that call fails), and `hints` (agent-safe patterns; when the project has no workflow states the first hint says so and shows `states create`, and when the project is archived the first hint says so). Cycle `status` (`draft`, `upcoming`, `current`, `completed`) is computed by plane-cli from the dates; `cycles list` and `cycles get` add the same field to each cycle. Checked against Plane CE 1.4.2.

`complete states --json` and `complete labels --json` add rich arrays alongside `names`:

- `states` — `{ id, name, group, color, sequence, default }` per row; `by_group` mirrors the bucket layout.
- `labels` — `{ id, name, color, description, sort_order }` per row.

Line-oriented stdout (without `--json`) is unchanged for shell tab completion.

## Inbox fetch (`inbox`)

`plane-cli --json [--as <member>] inbox` returns the work items assigned to the acting member (the key's own member without `--as`), as two capped lists, each ordered by priority then oldest `created_at`: `todo` (default 3), new work in the `unstarted` group only (In Progress is excluded, so an item already picked up is not fetched again; Backlog only with `--include-backlog`), and `blocked_reassess` (default 1, items in the Blocked state, `--blocked-state`). Done and Cancelled are never returned. `data.empty` is `true` when both are empty and no project was skipped; that is `ok: true` and exit 0, so a caller can skip the agent turn. A project skipped on 403/404 sets `data.partial: true` and is listed in `data.skipped_projects`; partial is never empty. `--project`, `--limit`, `--blocked-limit` and `--handlers PATH` (per-project `states` for `todo`, never for the Blocked pass, and an optional `blocked_state`) adjust it. Assignment only: CE 1.4.2 has no notifications endpoint for mentions. See [Inbox](inbox.md) and the [CRUD reference](https://github.com/pseudovar/plane-cli/blob/main/skills/plane-cli/CRUD-REFERENCE.md). Checked against Plane CE 1.4.2.

## Project write policy (`config policy`, `--dry-run`)

A TOML policy per project identifier (`.plane-cli/policy/<P>.toml` in the repository, else `<config dir>/plane-cli/policy/<P>.toml`), written with `plane-cli config policy init --project P` and shown with `config policy show --project P` and in `projects context` (`data.policy`). Repo-local files are merged over the global one and can only tighten it (see [Write policy](policy.md)). Before `work-items create|update|delete`, `intake create|update|delete`, comment, link and attachment deletes, `projects|modules|cycles delete`, `projects|modules|cycles archive`, and TUI edits and deletes, plane-cli checks its rules: `search_before_create`, `description_shape`, `required_fields`, `allowed_labels`, `comment_on_change` (needs `--reason`, which plane-cli posts as a comment), `state_permissions` and `no_deletes` (which also covers project, module and cycle deletes and archives, because archiving hides work; unarchive only restores and is not checked). The caller for member rules is the `--as` member, else the API key's own member. A refusal is `ok: false`, `error.code: "policy_violation"`, exit 2, with `error.details.rule`, `fix` and `violations` (every failed rule); nothing is written. `--dry-run` on those three commands returns `ok: true` with `data.allowed`, `data.checks` and the request, and writes nothing. Full rules and shapes in [Write policy](policy.md). Checked against Plane CE 1.4.2.

Not covered: writes other than those listed (comment and link creates, relations, module and cycle membership) are not checked. Policy refusals (`policy_violation`, `policy_invalid`, `policy_unknown_state`) are expected outcomes and are not sent to Sentry.

## Project setup commands

Checked against Plane CE 1.4.2; full shapes and CE quirks in the [CRUD reference](https://github.com/pseudovar/plane-cli/blob/main/skills/plane-cli/CRUD-REFERENCE.md).

- `projects features --id P [--cycles|--modules|--pages|--intake|--views on|off]` → `data: { id, identifier, features: { cycles, modules, pages, intake, views }, updated: [...] }`.
- `states create` warns when it fills in a colour, when it re-applies `--sequence` (CE ignores it on create), and when `--default` clears the flag on another state (CE does not). If one of those follow-up updates fails, the result stays `ok: true` with a warning.
- Label and state writes drop the cached catalog; if that fails, the write still succeeds with a warning to pass `--refresh`.
- List commands always return the paginated `{ "results": [...] }` shape, even where CE answers with a bare array (`cycles list --status current`).
- `projects members add` returns the membership `id`; `projects members update|remove --membership` need that id, not the user id. `remove` deactivates (`is_active: false`).
- `projects archive|unarchive --id P` and `modules archive|unarchive --project P --module M` → `data: { id, archived }`. `--module` / `--cycle` take a UUID or exact name: `archive` searches active rows only, `unarchive` archived rows only, other cycle commands both (a name in both lists is an ambiguity error naming both ids; exact beats case-insensitive). Only completed or cancelled modules can be archived (HTTP 400, `validation_error`, exit 1). An archived project stays in `projects list` (with `archived_at`) and `projects get` works, but its work items, states, cycles and modules read as empty lists and single gets are 404 until it is unarchived. `modules list --archived` lists archived modules.
- `projects summary --id P` → `data: { counts: { issues, cycles, modules, members, states, labels, intakes, pages }, id, identifier, name }`.
- `invitations update --id I --role admin|member|guest` patches a pending invitation (follows the schema; not checked live, because no invitation was pending).
- Relations can be created and listed but not removed: the CE public API has no endpoint for removing one.
- `intake …` adds `status_name` beside Plane's numeric `status`; `members list`, `projects members list` and `invitations list` add `role_name` beside `role`.
- `work-items attachments create --file PATH` uploads in three calls (register, storage POST, mark uploaded) and does not echo the presigned form; `attachments get` returns a short-lived `download_url`.

## Shell completions

**Supported shells** (same names you pass to `completions`, or use `--list-shells`):

```bash
plane-cli completions --list-shells
```

**Generate a script:** if you omit the shell, the CLI picks one from `$SHELL` (basename: `zsh`, `fish`, `bash`, …); if `SHELL` is unset, it emits **bash** and prints a short note on stderr.

```bash
plane-cli completions              # infer from $SHELL (fallback bash)
plane-cli completions bash > ~/.plane-cli.bash
plane-cli completions zsh  > ~/.plane-cli.zsh
plane-cli completions fish > ~/.plane-cli.fish
```

**Install automatically (recommended on macOS/Linux):** writes the same script to conventional locations and, for **bash** and **zsh**, adds a small marked block to your rc file (or **fish** into `~/.config/fish/completions/` with no rc change). Elvish and PowerShell only get the file under the data directory; you still wire your profile yourself. Use `--dry-run` to preview.

```bash
plane-cli completions --install              # infer shell from $SHELL
plane-cli completions zsh --install
plane-cli completions --install --catalog-hooks
plane-cli completions bash --install --dry-run
```

- **Bash:** script at `$XDG_DATA_HOME/plane-cli/completions/plane-cli.bash` (default `~/.local/share/...`); appends a `source` line to `~/.bashrc`, or to `~/.bash_profile` if that exists and `.bashrc` does not.
- **Zsh:** `_plane-cli` in that same `completions` directory; **prepends** an `fpath=…` line to `~/.zshrc` (before `compinit` in a new shell) so the generated completion is found.
- **Fish:** `~/.config/fish/completions/plane-cli.fish` (or `$XDG_CONFIG_HOME/fish/completions/`).

Re-running `--install` refreshes the script file; if the rc file already contains the managed block, the rc file is not modified again (idempotent).

**Catalog-aware tab completion** (project identifiers, state names, module UUIDs) uses workspace cache/API. Emit optional shell snippets that call `plane-cli complete …` (bash and zsh helpers; **full** auto wiring is provided for **bash**):

```bash
plane-cli completions bash --catalog-hooks > ~/.plane-cli.bash
# or append hooks to an existing file you already generated
```

**Completion candidates** (one suggestion per line on stdout; requires `PLANE_*` like other commands; honors global `--refresh`):

- `plane-cli complete projects`
- `plane-cli complete states --project <PROJECT>`
- `plane-cli complete labels --project <PROJECT>`
- `plane-cli complete modules --project <PROJECT>` (module UUIDs)
- `plane-cli complete cycles --project <PROJECT>` (cycle names; `--json` adds id, dates, status)

With `--json`, these return the usual success envelope instead of plain lines.

For a manual install, see your shell’s completion documentation (e.g. `fpath` for zsh, `source` for bash), or use `completions --install` above.

Clap parse failures honor `--json` / `PLANE_CLI_JSON`: stdout gets the usual error envelope (`validation_error`) instead of clap prose.
