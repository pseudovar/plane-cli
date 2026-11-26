# plane-cli CRUD reference

Exact CLI shapes for agents. **Do not infer JSON from Plane REST docs** — use this file or `plane-cli <cmd> --help`.

Cross-cutting contract (envelope, env, exit codes): [agent contract](https://pseudovar.github.io/plane-cli/agent-contract.html).

## Conventions

### stdout envelope (`--json`)

Every command returns one JSON object:

```json
{ "ok": true, "data": { }, "next_cursor": "…", "truncated": true }
```

- **`data`** — API response body (object, array, or `null` on delete).
- **`next_cursor` / `truncated`** — present on some paginated lists; pass `--cursor` on the next call.
- On failure: `ok: false`, `error.code`, `error.message` (see agent-contract).

### `--body` (important)

`--body` takes a **file path** or **`-`** (read JSON from stdin). Pass JSON on stdin, not as an inline string.

- Inline JSON (`--body '{"name":"x"}'`, a value starting with `{` or `[` that parses) is accepted **with a warning**; switch to `--body -`.
- A path that does not exist returns `ok: false`, `error.code: body_file_not_found` (exit `2`) and names the path. A directory or unreadable file gives `body_file_unreadable`; JSON that does not parse (file, stdin, or inline) gives `body_invalid_json`.

```bash
# stdin
echo '{"labels":["bug"]}' | plane-cli --json --strict-catalog work-items create \
  --project ENG --name "Fix login" --body -

# file
plane-cli --json work-items update --project ENG --work-item ENG-42 --body ./patch.json
```

### Flag → body merge

When both `--body` and flags are set: **load `--body` first, then merge flags on top** (flags win on key conflicts). Shallow merge — one level of keys only.

### Identifiers

| Kind | Format | Example |
|------|--------|---------|
| Project | short id or UUID | `ENG`, `550e8400-…` |
| Work item | composite or UUID | `ENG-123` (preferred). A composite id names its project, so `--project` is optional on every `--work-item` command (`get`, `update`, `delete`, `comments`, `links`, `relations`, `activities`). A UUID needs `--project`. |
| Composite id | `{project_identifier}-{sequence_id}` | From `data.identifier` + `data.sequence_id` |
| Module | UUID only | From `complete modules --project ENG` |
| Cycle | UUID or exact name | `"Sprint 12"`; from `complete cycles --project ENG` |
| Nested resource | UUID | comment/link/activity id from list/get |

### `--project` with `--work-item`

Checked against CE 1.4.2.

| Call | Result |
|------|--------|
| `--work-item ENG-123` (no `--project`) | Supported; project taken from the prefix. No warning. |
| `--project ENG --work-item ENG-123` | Supported. |
| `--project WEB --work-item ENG-123` | `ok: false`, `error.code: project_mismatch` (exit `2`). Nothing is changed. |
| `--work-item <UUID>` (no `--project`) | `ok: false`, `error.code: project_required_for_uuid` (exit `2`); use `ENG-123` or add `--project`. |
| `comments list ENG-123` (positional) | Accepted with one `warnings[]` entry; pass `--work-item` instead. |

The examples below show `--project ENG --work-item ENG-123`; with a composite id the `--project ENG` part may be dropped.

### Paginated list shape

Most list commands return `data` like:

```json
{
  "results": [ { "id": "…", "name": "…" } ],
  "next_cursor": "20:1:0",
  "count": 20,
  "total_results": 150
}
```

Use `--page-size N`, `--cursor CURSOR`, or `--all` (global list flags). When `truncated: true` on the envelope, more pages exist.

### Work item object (read)

Key fields agents use from `data` (single item or each entry in `results`):

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "sequence_id": 123,
  "name": "Fix login",
  "description": "…",
  "priority": "high",
  "state": { "id": "…", "name": "In Progress", "group": "started" },
  "assignees": [ { "id": "…", "display_name": "…" } ],
  "labels": [ { "id": "…", "name": "bug" } ],
  "created_at": "2024-01-15T10:30:00Z"
}
```

Display id: `{project_identifier}-{sequence_id}` (e.g. `ENG-123`).

---

## Global flags

| Flag | Scope |
|------|--------|
| `--json` | Machine-readable stdout (required for agents) |
| `--refresh` | Bypass cache for catalog-backed reads |
| `--strict-catalog` | On **work-items create/update only**: resolve `state` / `labels` names → UUIDs before POST/PATCH |
| `--env-file PATH` | Load env before config |
| `--dotenv` | Load `./.env` |
| `--as MEMBER` | Act as this Plane member with its own key (`PLANE_API_KEY_<MEMBER>` in the process env, then Keychain `plane-cli-<member>` on macOS or `~/.config/plane-cli/keys/<member>`); checked against `users/me` first. Also `PLANE_CLI_AS`. See [Agents and `--as`](https://pseudovar.github.io/plane-cli/agent-identity.html) |

---

## auth

### whoami

```bash
plane-cli --json auth whoami
plane-cli --json --as bob auth whoami
```

**Response `data`:** current user object from Plane (`id`, `email`, `display_name`, …). With `--as`, the member's user; a key that belongs to someone else fails with `member_identity_mismatch` before the command itself runs.

---

## config set-key

Store a member's API key for `--as`, without echoing it.

```bash
plane-cli config set-key --member bob            # hidden prompt on a TTY
<password-manager-cli> | plane-cli --json config set-key --member bob   # or stdin
```

| Flag | Required | Meaning |
|------|----------|---------|
| `--member MEMBER` | yes | Plane member name (`a-z`, `0-9`, `-`; no `_`) |

Writes the Keychain item `plane-cli-<member>` on macOS, else `~/.config/plane-cli/keys/<member>` (mode 600, directory 700). Replaces an existing key.

**Response `data`:** `member`, `stored_in` (`kind`: `keychain` with `service`, or `key_file` with `path`), `identity_pin_reset` (the remembered Plane user for this member was cleared, so a replaced account is accepted on next use), `next_step`. The key is never in the output.

**Errors:** `invalid_member_name`, `invalid_api_key` (nothing stored), `keychain_error`, `member_key_dir_insecure`, `member_key_store_unsupported`.

`config doctor` with `--as` reports `data.identity` (`member`, `key_source`, `verified`). It also warns when `PLANE_API_KEY` comes from a plain-text env file (`data.api_key_source`).

---

## config policy (per-project write policy)

Rules plane-cli enforces on work-item writes in one project, so agents get a refusal with the fix instead of relying on prose. Files are written with `config policy init`; do not edit them by hand. Checked against Plane CE 1.4.2.

```bash
plane-cli --json config policy init --project WEB [--owner alice ...] [--require priority --require module ...] \
  [--label bug ...] [--local] [--overwrite]
plane-cli --json config policy show --project WEB
```

| Flag | Meaning |
|------|---------|
| `--project P` | Project identifier (not a UUID); the file is `<P>.toml` |
| `--owner M` | Member allowed to set Todo, Done, Cancelled and to delete (repeatable). Without it those two rules are written commented out |
| `--require F` | Field `work-items create` must set (repeatable): `priority`, `module`, `labels`, `assignees`, `state`, `start_date`, `target_date`. Default `priority` |
| `--label L` | Allowed label name (repeatable). Without it any label is allowed |
| `--local` | Write `./.plane-cli/policy/<P>.toml` (repo-local override) instead of `<config dir>/plane-cli/policy/<P>.toml` |
| `--overwrite` | Replace an existing file (else `policy_exists`) |

**Lookup and merge:** the global file `$XDG_CONFIG_HOME/plane-cli/policy/<P>.toml` (`~/.config/…`) is read first. Then every repo-local `.plane-cli/policy/<P>.toml` from the current directory up to the repository root (the directory holding `.git`) or `$HOME`, whichever comes first, is merged over it, outermost first. When neither is above the current directory, only the current directory is searched. A repo-local file can only **tighten** the policy:

- required fields, description sections and `comment_on_change` fields are unions; `forbid_single_paragraph` is on if any file turns it on;
- allowed labels and delete members are intersections (`[labels] allowed = []` allows no labels; `[deletes] members = []` lets nobody delete);
- the duplicate threshold takes the lower value (lower is stricter: more names count as matches);
- state rules accumulate, and a state covered by several rules can only be set by a member listed in all of them;
- no rule is ever dropped (a local file with only `version = 1` changes nothing).

A repo-local file is skipped, with a warning, when the file, its `policy` directory or its `.plane-cli` directory is a symlink, is not owned by you, or is writable by group or others. A file that does not parse fails every write to that project with `policy_invalid` (reads are unaffected).

**Rules** (a section that is absent is not enforced):

| Rule (`error.details.rule`) | Section | Refuses |
|------|---------|---------|
| `search_before_create` | `[search_before_create] threshold = 0.85` | A create whose name is at least `threshold` similar to an open item in the project, unless `--allow-duplicate`. Similarity is the share of words both names have (order-free, counting repeats, one typo per word allowed, filler words like "the" ignored); names whose numbers differ ("v0.2.0" / "v0.2.1", two dates) never match. Done/Cancelled matches only give a warning. CE search is a substring match, so plane-cli searches the full name and its two longest words |
| `description_shape` | `[description] required_sections`, `forbid_single_paragraph` | A written description (create, or update with `--description` / `description_html`) without each section as a heading (`<h1>`-`<h6>`, or a paragraph that is only bold text; `"A\|B"` accepts either), or one that is a single plain paragraph. A create with no description fails too |
| `required_fields` | `[required_fields] on_create` | A create missing a listed field (`priority` must not be `none`; `module` is `--module`) |
| `allowed_labels` | `[labels] allowed` | Labels outside the set, on create or update (an empty list allows none) |
| `comment_on_change` | `[comment_on_change] fields` | A change of `state`, `assignees` or `priority` without `--reason` |
| `state_permissions` | `[[state_rules]] states` and/or `groups`, `members` | Moving an item into a listed state, or any state of a listed group (`backlog`, `unstarted`, `started`, `completed`, `cancelled`, `triage`), by a member not in `members`; on update, and on create including the project's default state when `--state` is omitted. A state plane-cli cannot find, even after refreshing the states catalog, is refused with `policy_unknown_state` |
| `no_deletes` | `[deletes] members` | `work-items delete`, `intake delete`, comment, link and attachment deletes, `projects delete`, `modules delete`, `cycles delete`, and `projects archive` / `modules archive` / `cycles archive` (archiving hides work) by a member not listed (`members = []`: nobody). Unarchive only restores, so it is not gated |

The caller is the `--as` member, else the API key's own member (`users/me`). Members match the Plane display name (the email local part only when the display name is empty), the same rule as `--as`.

**Where it applies:** `work-items create|update|delete`, `intake create` (the create checks on the submitted item; intake cannot set a module, so a policy that requires one refuses intake creates), `intake update` (the update checks on `--name`/`--description`/`--priority`; `--reason` is accepted and posted as a comment on the work item), `intake delete`, comment/link/attachment deletes, `projects|modules|cycles delete`, `projects|modules|cycles archive`, and TUI edits and deletes (a refusal shows in the status line and nothing is sent; the TUI has no reason prompt, so a change that needs `--reason` is refused with a pointer to the CLI). Not checked: comment and link creates, relations, module and cycle membership.

**`config policy show` `data`:** `{ "project", "active", "policy": { "project", "file", "source", "files": [{ "path", "source" }], "rules": {…}, "summary": ["…"] } | null, "searched": [{ "path", "source": "global"|"repo", "status": "merged"|"missing"|"skipped", "reason" }] }`. `file`/`source` are the most specific merged file; `files` lists every merged file in merge order. `init` returns `{ "project", "written", "source", "summary" }`.

**Errors:** `invalid_project_identifier`, `policy_exists`, `policy_invalid`, `invalid_member_name` (`--owner`); on writes `policy_violation` and `policy_unknown_state`.

### Refused writes

```json
{
  "ok": false,
  "error": {
    "code": "policy_violation",
    "message": "invalid usage: policy for WEB refused work-items create: description_shape: the description is a single plain paragraph",
    "details": {
      "rule": "description_shape",
      "fix": "send the description as HTML through --body with description_html, one <h3> per section (…)",
      "violations": [ { "rule": "description_shape", "result": "fail", "message": "…", "fix": "…" },
                      { "rule": "required_fields", "result": "fail", "message": "create is missing priority, module, labels", "fix": "…" } ],
      "project": "WEB",
      "policy_file": "…/plane-cli/policy/WEB.toml",
      "hint": "…"
    }
  }
}
```

Exit `2`; nothing was written. Fix every entry in `violations`, then retry (`--dry-run` first if unsure).

### `--dry-run`

`work-items create|update|delete --dry-run` runs the policy (including the duplicate search, a read) and writes nothing. It works with or without a policy.

```json
{ "ok": true, "data": { "dry_run": true, "action": "work-items update", "project": "WEB", "allowed": false,
  "policy": { "file": "…", "source": "global" },
  "checks": [ { "rule": "comment_on_change", "result": "fail", "message": "this changes state without --reason", "fix": "…" } ],
  "request": { "method": "PATCH", "path": "/api/v1/workspaces/<slug>/projects/<id>/work-items/<id>/", "body": { "state": "<uuid>" } },
  "then": [ { "step": "comment", "comment_html": "…" } ],
  "changes": [ { "field": "state", "before": "Backlog", "after": "In Progress" } ] } }
```

`allowed: false` means the real write would be refused; the dry run itself is `ok: true`. `changes` is on update only; `then` lists the follow-up calls (`module_add` on create, `comment` on update).

---

## projects

### list

```bash
plane-cli --json projects list [--page-size N] [--cursor C] [--all]
```

**Response `data`:** paginated; each result has `id`, `name`, `identifier` (short id, e.g. `ENG`).

### get

```bash
plane-cli --json projects get --id ENG
```

**Response `data`:** single project object.

### create

```bash
plane-cli --json projects create \
  [--name "Name"] [--description "…"] [--identifier SHORT] [--project-lead UUID] \
  [--body PATH|-]
```

| Flag | JSON field |
|------|------------|
| `--name` | `name` |
| `--description` | `description` |
| `--identifier` | `identifier` |
| `--project-lead` | `project_lead` |

**Response `data`:** created project object.

### update

```bash
plane-cli --json projects update --id ENG \
  [--name "…"] [--description "…"] [--identifier SHORT] [--project-lead UUID] \
  [--body PATH|-]
```

Same flag → field mapping as create. **Response `data`:** updated project.

### delete

```bash
plane-cli --json projects delete --id ENG
```

**Response `data`:** `null`. Refused by a `[deletes]` policy rule for members not listed (`policy_violation`, rule `no_deletes`).

### archive / unarchive

Checked against Plane Community Edition 1.4.2 (archived, read, and unarchived in a test project).

```bash
plane-cli --json projects archive --id ENG      # data: { "id", "archived": true }
plane-cli --json projects unarchive --id ENG    # data: { "id", "archived": false }
```

How an archived project behaves on CE 1.4.2 (the schema says it is hidden from project lists; it is not):

- It **stays in `projects list`** with `archived_at` set, so its identifier still resolves. `projects get`, `projects members list` and `projects summary` work as before.
- **Work-item reads do not work:** `work-items list` returns an empty list and `work-items get` (UUID or `ENG-123`) answers 404. `states list`, `cycles list` and `modules list` are empty too. `projects context` still answers, and its first hint says the project is archived.
- `projects unarchive` brings everything back: same feature flags, members and roles, states and work items (only `updated_at` changes).

`projects archive` is refused by a `[deletes]` policy rule for members not listed (see [config policy](#config-policy-per-project-write-policy)).

### summary

Checked against Plane Community Edition 1.4.2 (read only).

```bash
plane-cli --json projects summary --id ENG
```

**Response `data`:** `{ "counts": { "issues", "cycles", "modules", "members", "states", "labels", "intakes", "pages" }, "id", "identifier", "name" }`. The same counts are in `projects context` as `summary.counts`.

### features

Checked against Plane Community Edition 1.4.2 (all five switched on, one off and on again, in a test project).

```bash
plane-cli --json projects features --id ENG                          # show
plane-cli --json projects features --id ENG --cycles on --modules on --intake on --views on --pages on
plane-cli --json projects features --id ENG --views off
```

| Flag | Project field |
|------|---------------|
| `--cycles on\|off` | `cycle_view` |
| `--modules on\|off` | `module_view` |
| `--pages on\|off` | `page_view` |
| `--intake on\|off` | `intake_view` |
| `--views on\|off` | `issue_views_view` |

`on|off` also accepts `true|false`, `yes|no`, `1|0`. With no switch flags the command only reads.

**Response `data`:** `{ "id", "identifier", "features": { "cycles", "modules", "pages", "intake", "views" }, "updated": ["cycles", …] }`.

### members

Checked against Plane Community Edition 1.4.2 (add, role change, remove of one workspace member in a test project).

```bash
plane-cli --json projects members list --project ENG [--include-inactive]
plane-cli --json projects members add --project ENG --member bob [--role member]   # UUID, email or display name
plane-cli --json projects members update --project ENG --membership <MEMBERSHIP_UUID> --role guest
plane-cli --json projects members remove --project ENG --membership <MEMBERSHIP_UUID>
```

`--role` is `admin`, `member` (default) or `guest` (Plane's 20, 15, 5).

**Response `data`:** `list` is paginated; each row is the **user** (`id` is the user id) with `role`, `role_name`, `is_active`, `display_name`, `email`. Removed members are hidden unless `--include-inactive`; `count` stays Plane's (all memberships on the page) and plane-cli adds `active_count`. `add` and `update` return `{ "id": "<MEMBERSHIP_UUID>", "member": "<USER_UUID>", "role", "role_name" }`. `remove` returns `{ "id", "member", "is_active": false }`.

How CE 1.4.2 behaves:

1. **Update and remove need the membership id, and only `add` returns it.** No list endpoint exposes membership ids, so members added in the Plane UI cannot have their role changed or be removed through the API. Passing a user id gets a `validation_error` that says so.
2. **Remove deactivates.** The membership stays (with `is_active: false`) and Plane's own `project-members/` list keeps showing the user; plane-cli lists from `project-members-lite/`, which carries `is_active`, and hides inactive rows by default.
3. **A removed member cannot be added back through the API** (`400 The payload is not valid`); plane-cli checks first and says to use the Plane UI.

---

## work-items

### list

```bash
plane-cli --json work-items list --project ENG [--page-size N] [--cursor C] [--all] \
  [--state "In Progress"] [--assignee <UUID>]
```

| Flag | Behaviour |
|------|-----------|
| `--project ENG` | Required. Project identifier or UUID. |
| `--state NAME\|UUID` | Filter by state name or UUID (client-side; names resolved automatically — no `--strict-catalog` needed). |
| `--assignee USER` | Filter by assignee (repeatable): user UUID, or workspace member display name or email; an unknown or ambiguous name is a `validation_error` (exit 2). |

**Response `data`:** paginated work items (`results` array). When filters are active, a `warnings` field is included in the envelope noting client-side filtering.

### get

```bash
plane-cli --json work-items get --work-item ENG-123
plane-cli --json work-items get --work-item <UUID> --project ENG
```

**Response `data`:** single work item object.

### create

```bash
plane-cli --json [--strict-catalog] work-items create --project ENG \
  [--name "Title"] [--description "…"] [--priority PRI] [--state STATE] \
  [--assignee UUID ...] [--module MODULE] [--allow-duplicate] [--dry-run] [--body PATH|-]
```

| Flag | JSON field | Notes |
|------|------------|-------|
| `--name` | `name` | |
| `--description` | `description_html` | CLI accepts plain text and writes Plane's HTML description field |
| `--priority` | `priority` | `none`, `low`, `medium`, `high`, `urgent` |
| `--state` | `state` | UUID or state name; use `--strict-catalog` for names |
| `--assignee` | `assignees` | Repeatable; builds `["uuid", …]` |
| `--body` | any | **Only way to set `labels`** (and other API fields without flags) |
| `--module` | — | Module UUID or exact name; the item is added to it after create (`data.module_id`). If that step fails, `error.details` has `step: "module_add"` and the created id |
| `--allow-duplicate` | — | Pass the project policy's `search_before_create` rule (a warning names the close matches) |
| `--dry-run` | — | Show the request and the policy checks; nothing is created (see [config policy](#config-policy-per-project-write-policy)) |

When the project has a policy, the create is checked first and refused with `policy_violation` if it breaks a rule.

**`labels` in `--body`:** JSON array of label names or UUIDs, e.g. `{"labels":["bug","tech-debt"]}`. With `--strict-catalog`, names are resolved to UUIDs.
**`description` in `--body`:** if present, it is normalized to Plane's `description_html` field for work items.

**Example (labels via stdin):**

```bash
echo '{"labels":["bug"]}' | plane-cli --json --strict-catalog work-items create \
  --project ENG --name "Fix login" --state Todo --body -
```

**Response `data`:** created work item object.

### update

```bash
plane-cli --json [--strict-catalog] work-items update --project ENG --work-item ENG-123 \
  [--name "…"] [--description "…"] [--priority PRI] [--state STATE] \
  [--assignee UUID ...] [--reason TEXT] [--dry-run] [--body PATH|-]
```

Same flag → field mapping as create. Omit flags for fields that should stay unchanged.

- `--reason TEXT` — plain text posted as a comment after the update, followed by a list of the state, priority and assignee changes (`<p>reason</p><ul><li><strong>state</strong>: Todo → In Progress</li></ul>`). `data.reason_comment` has the comment `id` and `comment_html`. A project policy with `comment_on_change` refuses those changes without it. If the comment fails after the update, the result is still `ok: true` (the update stands) with a warning, `data.reason_comment: null` and `data.reason_comment_error: { "step": "reason_comment", "code", "comment_html" }`; post it again with `work-items comments create`.
- `--dry-run` — show the request, the field changes (read from the current item, with or without a policy), the comment and the policy checks; nothing is written.

**Clear all assignees:** `--body -` with stdin `{"assignees":[]}` (no `--assignee` flags).

**Update description only:**

```bash
plane-cli --json work-items update --project ENG --work-item ENG-123 \
  --description "Knowledge resource: https://example.com/doc"
```

**Update description and labels together:** labels still go through **`--body -`**; `--description` stays a flag.

```bash
echo '{"labels":["bug","tech-debt"]}' | plane-cli --json --strict-catalog work-items update \
  --project ENG --work-item ENG-123 \
  --description "Knowledge resource: https://example.com/doc" \
  --body -
```

**Wrong:** `--labels bug` or positional ids like `work-items update ENG ENG-123 ...`

**Correct:** always pass `--project` / `--work-item`; use `--body -` for labels.

**Response `data`:** updated work item object.

### delete

```bash
plane-cli --json work-items delete --project ENG --work-item ENG-123 [--dry-run]
```

**Response `data`:** `null`. A project policy with `[deletes]` refuses deletes by members not listed (`policy_violation`, rule `no_deletes`); the same check runs on comment, link and attachment deletes.

### search

```bash
plane-cli --json work-items search --query "login" [--project ENG] [--limit N]
```

**Response `data`:** object with `results` array of work items, or a bare array (handle both).

---

## work-items comments

### list / get

```bash
plane-cli --json work-items comments list --project ENG --work-item ENG-123
plane-cli --json work-items comments get --project ENG --work-item ENG-123 --id <COMMENT_UUID>
```

**Response `data`:** paginated list or single comment (`id`, `comment_html`, `created_at`, …).

### create

```bash
plane-cli --json work-items comments create --project ENG --work-item ENG-123 \
  [--comment-html "<p>…</p>"] [--body PATH|-]
```

| Flag | JSON field |
|------|------------|
| `--comment-html` | `comment_html` |

**Agent-safe example (`--body -`):**

```bash
echo '{"comment_html":"<p>Looks good</p>"}' | plane-cli --json work-items comments create \
  --project ENG --work-item ENG-123 --body -
```

**Response `data`:** created comment.

### update

```bash
plane-cli --json work-items comments update --project ENG --work-item ENG-123 --id <UUID> \
  [--comment-html "…"] [--body PATH|-]
```

**Response `data`:** updated comment.

### delete

```bash
plane-cli --json work-items comments delete --project ENG --work-item ENG-123 --id <UUID>
```

The comment is addressed with `--id`. `--comment-id` is accepted on `get` / `update` / `delete` with a warning (likewise `--link-id` on `links`).

**Response `data`:** `null`.

---

## work-items links

### list / get

```bash
plane-cli --json work-items links list --project ENG --work-item ENG-123
plane-cli --json work-items links get --project ENG --work-item ENG-123 --id <LINK_UUID>
```

### create

```bash
plane-cli --json work-items links create --project ENG --work-item ENG-123 \
  [--url "https://…"] [--title "…"] [--body PATH|-]
```

| Flag | JSON field |
|------|------------|
| `--url` | `url` |
| `--title` | `title` |

**Response `data`:** created link.

### update / delete

```bash
plane-cli --json work-items links update --project ENG --work-item ENG-123 --id <UUID> \
  [--url "…"] [--title "…"] [--body PATH|-]
plane-cli --json work-items links delete --project ENG --work-item ENG-123 --id <UUID>
```

---

## work-items relations

Relations are **create + list only**. A relation cannot be removed with plane-cli: the CE 1.4.2 public API has no endpoint for removing one, so remove it in the Plane UI.

### list

```bash
plane-cli --json work-items relations list --project ENG --work-item ENG-123
```

**Response `data`:** grouped by relation type, e.g.:

```json
{
  "blocking": ["uuid-…"],
  "blocked_by": ["uuid-…"],
  "duplicate": [],
  "relates_to": ["uuid-…"]
}
```

### create

```bash
plane-cli --json work-items relations create --project ENG --work-item ENG-123 \
  [--relation-type TYPE] [--issues UUID,UUID,...] [--body PATH|-]
```

| Flag | JSON field |
|------|------------|
| `--relation-type` | `relation_type` |
| `--issues` | `issues` | Comma-separated work item UUIDs |

**`relation_type` values:** `blocking`, `blocked_by`, `duplicate`, `relates_to`, `start_before`, `start_after`, `finish_before`, `finish_after`.

**Example:**

```bash
plane-cli --json work-items relations create --project ENG --work-item ENG-123 \
  --relation-type blocked_by --issues 550e8400-e29b-41d4-a716-446655440099
```

**Response `data`:** relation create response from API.

---

## work-items attachments

Checked against Plane Community Edition 1.4.2 (upload, list, download, delete of a text file in a test project).

```bash
plane-cli --json work-items attachments list --project ENG --work-item ENG-123
plane-cli --json work-items attachments create --project ENG --work-item ENG-123 --file ./report.pdf [--name "Report.pdf"] [--type application/pdf]
plane-cli --json work-items attachments get --project ENG --work-item ENG-123 --id <ATTACHMENT_UUID> [--output ./report.pdf]
plane-cli --json work-items attachments delete --project ENG --work-item ENG-123 --id <ATTACHMENT_UUID>
```

Like the other `--work-item` commands, `--project` is optional with a `PREFIX-N` id and required with a UUID. `create` does Plane's three-step upload: register the file (`POST attachments/` with name, MIME type and size, which returns a presigned storage form), send the bytes straight to storage, then mark the attachment uploaded (`PATCH is_uploaded`). The MIME type is guessed from the file name unless `--type` is given (only `text/plain` was tried live; the instance may refuse types outside its allowed list).

**Response `data`:** `list` is an array of attachment objects (`id`, `attributes: { name, size, type }`, `size`, `is_uploaded`, `asset`, …). `create` returns `{ "asset_id", "asset_url", "attachment": { …, "is_uploaded": true } }` (the presigned form is not echoed). If the storage upload or the final `PATCH` fails, plane-cli deletes the registered attachment; the error keeps its code and adds `error.details` `{ "step", "asset_id", "cleaned_up" }` (with `cleanup_error` when the delete also failed, so the leftover can be deleted by id). A 2xx response without `asset_id` is `unexpected_response` (exit 1). `get` returns the attachment object plus `download_url`, a presigned storage link that expires; with `--output PATH` it also saves the file (refusing to overwrite) and adds `saved_to` and `saved_bytes`. `delete` returns `null`.

---

## work-items activities

Read-only audit trail.

```bash
plane-cli --json work-items activities list --project ENG --work-item ENG-123
plane-cli --json work-items activities get --project ENG --work-item ENG-123 --id <ACTIVITY_UUID>
```

---

## inbox (inbox fetch)

Work items assigned to the acting member (`--as`, else the key's own member from `users/me`), across every project the member belongs to, as two capped lists: new work (`todo`) and one Blocked item to reassess (`blocked_reassess`). Checked against Plane CE 1.4.2.

```bash
plane-cli --json [--as <member>] inbox [--project P] [--limit 3] [--blocked-limit 1] \
  [--include-backlog] [--blocked-state Blocked] [--handlers PATH]
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--project P` | all member projects | Scan one project only (identifier or UUID) |
| `--limit N` | 3 | Most items in `todo` (the batch cap) |
| `--blocked-limit N` | 1 | Most items in `blocked_reassess`; `0` skips the Blocked pass |
| `--include-backlog` | off | Also treat the `backlog` state group as new work |
| `--blocked-state NAME` | `Blocked` | State name of the Blocked pass (case-insensitive) |
| `--handlers PATH` | none | A JSON file: `{"projects": {"OPS": {"states": ["Backlog", "Needs Triage"], "blocked_state": "On Hold"}}}`. A listed project's `states` replace the default new-work rule for `todo`; `blocked_state` replaces `--blocked-state` for that project |

**Which items:**

- Never: terminal state groups (`completed`, `cancelled`).
- `blocked_reassess`: items whose state name is the Blocked state, whatever its group. The handlers file's `states` never filter this pass.
- `todo` (new work): the `unstarted` group (Todo) only, plus `backlog` with `--include-backlog`. Items in the `started` group (In Progress) are left out: an item moves to In Progress when the agent acts on it, so it is not fetched again. For a project listed in `--handlers`, its `states` decide instead (so a project may list "In Progress").
- Each list is ordered by priority (`urgent`, `high`, `medium`, `low`, `none`), then oldest `created_at`, then the cap is applied.

**Response `data`:**

```json
{
  "empty": false,
  "partial": false,
  "skipped_projects": [],
  "member": { "id": "<uuid>", "display_name": "bob", "acting_as": "bob" },
  "todo": [
    { "id": "<uuid>", "composite_id": "ENG-105", "name": "…", "project": "ENG", "project_id": "<uuid>",
      "state": { "id": "<uuid>", "name": "Todo", "group": "unstarted" }, "priority": "urgent",
      "created_at": "…", "updated_at": "…", "target_date": null, "handler_skill": "plane-eng-inbox" }
  ],
  "blocked_reassess": [],
  "counts": { "assigned_open": 14, "todo": 5, "blocked": 0, "skipped_in_progress": 0, "skipped_backlog": 9,
              "skipped_other": 0, "skipped_by_handlers": 0 },
  "limits": { "todo": 3, "blocked": 1 },
  "rules": { "todo_groups": ["unstarted"], "blocked_state": "Blocked", "handlers": null },
  "projects_scanned": ["ENG", "WEB"]
}
```

- `empty: true` (both lists empty, nothing skipped) is `ok: true`, exit 0, so a scheduler can skip the agent turn.
- **Partial is not empty.** A project the member cannot read (HTTP 403/404) is skipped: it is listed in `skipped_projects` (`{ "project", "code" }`), `partial` is `true`, a warning names it, and `empty` stays `false` even with both lists empty. A 5xx or network error fails the whole command (`ok: false`).
- `counts.assigned_open` is every open assigned item; `todo` / `blocked` are totals before the cap; the `skipped_*` counts say why the rest are not new work.
- `handler_skill` is a suggested name, `plane-{identifier lower-case}-inbox`, for a per-project skill or script you define yourself. plane-cli ships no such handlers and does not act on this field; ignore it if you do not use the convention.
- CE 1.4.2 has no server-side assignee filter, so the command lists each project's work items (`expand=state`) and filters locally: one call per 100 work items per project. A project with more than 5,000 work items (the list cap: 50 pages of 100) warns that some items may be missing.
- Mentions are not covered: CE 1.4.2's public API has no notifications endpoint.

---

## complete (catalog lookups)

Not CRUD — use before mutations when you need valid names/ids. **No `--strict-catalog` here.**

```bash
plane-cli --json complete projects
plane-cli --json complete states --project ENG
plane-cli --json complete labels --project ENG
plane-cli --json complete modules --project ENG
plane-cli --json complete cycles --project ENG
```

**Response `data` shapes:**

| Command | `data` fields |
|---------|----------------|
| `complete projects` | `{ "kind": "projects", "identifiers": ["ENG", …] }` |
| `complete states` | `{ "kind": "states", "project": "ENG", "names": ["Todo", …], "states": [{ "id", "name", "group", "color", "sequence", "default" }, …], "by_group": { "backlog", "unstarted", "started", "completed", "cancelled", "triage" } }` |
| `complete labels` | `{ "kind": "labels", "project": "ENG", "names": ["bug", …], "labels": [{ "id", "name", "color", "description", "sort_order" }, …] }` |
| `complete modules` | `{ "kind": "modules", "project": "ENG", "uuids": ["…", …] }` |
| `complete cycles` | `{ "kind": "cycles", "project": "ENG", "names": ["Sprint 1", …], "cycles": [{ "id", "name", "start_date", "end_date", "status", "archived" }, …] }` (active cycles only) |

`names` / line-oriented stdout remain for shell completion; prefer `states`, `by_group`, and `labels` in agent scripts.

Without `--json`, these print one value per line on stdout.

---

## projects context (agent preflight)

One-shot snapshot for a project: identity, feature flags, grouped workflow states, labels, optional module and cycle lists, and mutation hints. **Requires `--json`.** When the project has no workflow states, the first hint says so and shows `states create`.

```bash
plane-cli --json projects context --id ENG
```

**Response `data`:** `{ "kind": "project_context", "project": "ENG", "identity": { "id", "identifier", "name" }, "capabilities": { "cycle_view", "module_view", …, "archived" }, "states": { "backlog": […], … }, "states_flat": […], "labels": […], "modules": [{ "id", "name" }, …] | null, "cycles": [{ "id", "name", "start_date", "end_date", "status", "archived" }, …] | null, "summary": { "counts": { "issues", "cycles", "modules", "members", "states", "labels", "intakes", "pages" } } | null, "policy": { "project", "file", "source", "rules", "summary" } | { "error" } | null, "hints": […] }`

`policy` is the project's active write policy ([config policy](#config-policy-per-project-write-policy)); read `policy.summary` before writing. When a policy is active, the first hint says so.

`summary.counts` comes from [projects summary](#summary); if that call fails, `summary` is `null` and `warnings[]` says why (the rest of the snapshot is unaffected). When the project is archived (`capabilities.archived`), the first hint says so and the "no workflow states" hint is left out (an archived project reads as having none).

`cycles` is `null` when the project has cycles turned off (`capabilities.cycle_view` false). `status` is computed by plane-cli from the dates (see [cycles](#cycles)).

Use after `projects create` with `--refresh` if catalog resolution fails. Prefer this over multiple `complete *` calls when bootstrapping an agent session.

---

## modules

Modules are addressed by **UUID** (not short name), except `archive` and `unarchive`, whose `--module` also takes the exact name.

### list / get

```bash
plane-cli --json modules list --project ENG
plane-cli --json modules list --project ENG --archived
plane-cli --json modules get --project ENG --id <MODULE_UUID>
```

`--archived` lists archived modules (`GET archived-modules/`), each with `archived_at`. Archived modules are not in the plain list, and `modules get` answers 404 for them.

### create / update

**`--body` only** (no field flags):

```bash
plane-cli --json modules create --project ENG --body ./module.json
plane-cli --json modules update --project ENG --id <MODULE_UUID> --body ./module.json
```

Consult Plane API / OpenAPI for module body fields (`name`, `description`, dates, …).

### delete

```bash
plane-cli --json modules delete --project ENG --id <MODULE_UUID>
```

**Response `data`:** `null`. Refused by a `[deletes]` policy rule for members not listed.

### archive / unarchive

Checked against Plane Community Edition 1.4.2 (a test module: archive refused while `planned`, archived once `completed` and once `cancelled`, listed with `--archived`, unarchived by name, then deleted).

```bash
plane-cli --json modules archive --project ENG --module "Q4 launch"     # data: { "id", "archived": true }
plane-cli --json modules unarchive --project ENG --module "Q4 launch"   # data: { "id", "archived": false }
```

`--module` (alias `--id`) takes the module UUID or its exact name (a unique case-insensitive match also works). `archive` looks the name up in active modules only and `unarchive` in archived modules only. **Only completed or cancelled modules can be archived**; otherwise Plane answers `400 Only completed or cancelled modules can be archived` (`validation_error`, exit 1, like every HTTP error). Plane also refuses a new module whose name an archived module already has (`MODULE_NAME_ALREADY_EXISTS`), but names differing only in case are allowed; plane-cli prefers the exact match. Set the status first: `echo '{"status":"completed"}' | plane-cli --json modules update --project ENG --id <MODULE_UUID> --body -`. `modules archive` is refused by a `[deletes]` policy rule for members not listed.

---

## modules issues

Attach or detach work items from a module.

### list

```bash
plane-cli --json modules issues list --project ENG --module <MODULE_UUID>
```

**Response `data`:** paginated work items in the module.

### add

```bash
plane-cli --json modules issues add --project ENG --module <MODULE_UUID> \
  --work-items ENG-12,ENG-13
```

`--work-items` takes composite ids (`ENG-123`) or UUIDs, comma-separated. Composite ids must belong to `--project` (otherwise `project_mismatch`, exit `2`). All ids are resolved before anything is written. `--work-item` (singular) is accepted with a warning.

### remove

```bash
plane-cli --json modules issues remove --project ENG --module <MODULE_UUID> \
  --work-items ENG-12,ENG-13
```

Same `--work-items` list as `add` (`--work-item` is accepted with a warning). Repeated ids (including `ENG-12` and its UUID) are removed once. Items are removed one at a time; if one fails, the error keeps that failure's `error.code` and exit code, its message names the items already removed, and `error.details` is `{ "removed": [{ "input", "id" }, …], "failed": "<input>", "cause": <Plane error body, if any> }`.

**Response `data`:** `{ "module": "<MODULE_UUID>", "removed": [{ "input": "ENG-12", "id": "<UUID>" }, …] }`.

---

## labels

Checked against Plane Community Edition 1.4.2 (create, nest, rename, re-order, duplicate name, delete in a test project).

`--label` takes the label UUID or exact name (a unique case-insensitive match also works). Every write drops plane-cli's cached labels catalog, so `complete labels` and `--strict-catalog` see the change at once; if dropping the cache fails, the write still reports `ok: true` with a warning to use `--refresh`.

```bash
plane-cli --json labels list --project ENG
plane-cli --json labels create --project ENG --name bug --color '#EF4444' [--description "…"] [--parent <LABEL>] [--sort-order N]
plane-cli --json labels update --project ENG --label bug --name defect --color F97316
plane-cli --json labels delete --project ENG --label defect          # data: null
```

`--color` takes `#RRGGBB` or `#RGB` (the `#` is optional; plane-cli upper-cases it). `--parent` nests the label under another label (UUID or name). `--body PATH|-` is loaded first and the flags merged on top. A duplicate name is `409 conflict`, and `error.details.id` is the existing label's id.

**Response `data`:** `list` is paginated label objects (`id`, `name`, `color`, `description`, `parent`, `sort_order`, …); create and update return the label.

---

## states

Checked against Plane Community Edition 1.4.2 (a five-state workflow created in a test project; rename, recolour, re-order, default switch and delete).

`--state` takes the state UUID or exact name. Every write drops plane-cli's cached states catalog.

```bash
plane-cli --json states list --project ENG
plane-cli --json states create --project ENG --name "In Review" --group started [--color '#F59E0B'] [--sequence 40000] [--default] [--description "…"]
plane-cli --json states update --project ENG --state "In Review" --name Review --sequence 38000
plane-cli --json states update --project ENG --state Backlog --default
plane-cli --json states delete --project ENG --state Review           # data: null
```

`--group` is one of `backlog`, `unstarted`, `started`, `completed`, `cancelled` (and `triage`, which Plane reserves for intake). `create` needs `--name` and `--group`; without `--color` it uses Plane's default colour for the group and warns. A new project's default workflow, as Plane makes it:

```bash
plane-cli --json states create --project ENG --name Backlog --group backlog --color '#60646C' --sequence 15000 --default
plane-cli --json states create --project ENG --name Todo --group unstarted --color '#60646C' --sequence 25000
plane-cli --json states create --project ENG --name "In Progress" --group started --color '#F59E0B' --sequence 35000
plane-cli --json states create --project ENG --name Done --group completed --color '#46A758' --sequence 45000
plane-cli --json states create --project ENG --name Cancelled --group cancelled --color '#9AA4BC' --sequence 55000
```

**Response `data`:** `list` is paginated state objects (`id`, `name`, `group`, `color`, `sequence`, `default`, …); create and update return the state.

How CE 1.4.2 behaves:

1. **`sequence` is ignored on create** (Plane appends the state after the last one) but honoured on update. When `--sequence` is given, plane-cli applies it with a follow-up update and warns. If that follow-up fails, the result is still `ok: true` with the created state and a warning saying how to retry.
2. **Setting `default` with an update does not clear it elsewhere** (a create with `default: true` does), so a project can end up with two default states. After `--default`, plane-cli clears the flag on any other state and warns for each one (a failed clear is a warning too).
3. **The default state, and states that still hold work items, cannot be deleted** (`400 Default state cannot be deleted`).
4. A project with no states still accepts work items, with `state: null`; `projects context` adds a hint saying so.
5. The first intake item makes Plane add a hidden `Triage` state; `states list` does not show it.

---

## intake

Checked against Plane Community Edition 1.4.2 (create, get, rename, accept, mark duplicate, reject and delete in a test project).

The project needs intake turned on (`projects features --id ENG --intake on`). `--work-item` (alias `--id`) takes the intake item's composite id (`ENG-42`), its work item UUID, or the intake row's own `id`; plane-cli matches it against the intake list, because Plane addresses intake items by work item UUID and `work-items get` does not find items that are still pending.

```bash
plane-cli --json intake list --project ENG
plane-cli --json intake create --project ENG --name "Login fails on Safari" [--description "<p>…</p>"] [--priority high]
plane-cli --json intake get --project ENG --work-item ENG-42
plane-cli --json intake update --project ENG --work-item ENG-42 --status accepted
plane-cli --json intake update --project ENG --work-item ENG-43 --status duplicate --duplicate-to ENG-42
plane-cli --json intake update --project ENG --work-item ENG-44 --status snoozed --snoozed-till 2030-12-01T00:00:00Z
plane-cli --json intake update --project ENG --work-item ENG-42 --name "Login fails on Safari 18" --priority urgent
plane-cli --json intake delete --project ENG --work-item ENG-43       # data: null
```

`--status` is `pending`, `rejected`, `snoozed`, `accepted` or `duplicate` (Plane's -2, -1, 0, 1, 2). `--name`, `--description` (sent as `description_html`) and `--priority` edit the work item. `--body PATH|-` may be the whole request (`{"issue": {…}, "status": …}`) or bare work item fields; the flags are merged into `issue`.

**Response `data`:** intake rows `{ "id", "issue": "<WORK_ITEM_UUID>", "issue_detail": { "name", "sequence_id", "state", "priority", … }, "status", "status_name", "duplicate_to", "snoozed_till", "source", … }`; plane-cli adds `status_name`. New items start `pending` in the hidden `Triage` state; accepting moves the work item to the project's default state. Deleting an item that was not accepted also deletes its work item; deleting an accepted work item removes its intake row.

---

## members (workspace)

Checked against Plane Community Edition 1.4.2 (read only).

```bash
plane-cli --json members list
```

**Response `data`:** array of users with their workspace `role` (20 admin, 15 member, 5 guest); plane-cli adds `role_name`. Project membership is under [projects members](#members).

---

## invitations

Checked against Plane Community Edition 1.4.2: `list` only. `create`, `update` and `delete` follow the instance schema but were not run: `create` emails a real person, and the workspace had no pending invitation to update. `update` is **not checked live**; mock-server tests cover it.

```bash
plane-cli --json invitations list
plane-cli --json invitations create --email someone@example.com [--role member]
plane-cli --json invitations update --id <INVITATION_UUID> --role guest  # PATCH invitations/{id}/
plane-cli --json invitations delete --id <INVITATION_UUID>              # data: null
```

`--role` is `admin`, `member` or `guest` (or 20, 15, 5).

**Response `data`:** `list` is an array of invitations (`id`, `email`, `role`, `accepted`, …) with `role_name` added; `create` and `update` return the invitation.

---

## cycles

Checked against Plane Community Edition 1.4.2: create, rename, re-date, items add/list/remove, transfer, archive, unarchive and delete in a test project; list, get and items list read against a live project.

`--cycle` takes the cycle's UUID or its exact name (a unique case-insensitive match also works). Name lookup: `archive` searches active cycles only and `unarchive` archived cycles only; other commands search both lists, and a name found in both is an ambiguity error naming both ids (pass the UUID). An exact match beats a case-insensitive one across both lists. An unknown name lists the active and archived names that were searched. The project must have cycles turned on (`capabilities.cycle_view` in `projects context`); otherwise Plane answers `400 Cycles are not enabled for this project`.

### list / get

```bash
plane-cli --json cycles list --project ENG
plane-cli --json cycles list --project ENG --status current     # all|current|upcoming|completed|draft|incomplete
plane-cli --json cycles list --project ENG --archived
plane-cli --json cycles get --project ENG --cycle "Sprint 1"
```

**Response `data`:** paginated Plane cycle objects (`id`, `name`, `start_date`, `end_date`, `total_issues`, `completed_issues`, …). plane-cli adds `status` to each row: `draft` (no dates), `upcoming`, `current` or `completed`, judged from the dates against the current time (Plane's v1 API returns no status field). `--status` is passed to Plane as `cycle_view`. CE 1.4.2 answers `--status current` with a bare array instead of the paginated object; plane-cli wraps it so every `--status` value returns the same `{ "results": [...], "count", … }` shape.

### create

```bash
plane-cli --json cycles create --project ENG --name "Sprint 1" \
  --start 2030-12-01 --end 2030-12-14 [--description "..."] [--owner <USER_UUID>]
```

`--start` and `--end` are dates (`YYYY-MM-DD`), **both or neither** (a cycle with no dates is a draft). **There is no time-zone flag**: Plane stores the start at 00:00:01 and the end at 23:59 in the *project's* time zone (`projects get` → `timezone`), whatever the cycle's own `timezone` field says, so a flag would change nothing. Details under [How CE 1.4.2 behaves](#how-ce-142-behaves-checked-21-nov-2026). `--body PATH|-` is loaded first and the flags are merged on top, for other fields such as `external_source` and `external_id`.

### update

```bash
plane-cli --json cycles update --project ENG --cycle "Sprint 1" --name "Sprint 1 (holiday)"
plane-cli --json cycles update --project ENG --cycle "Sprint 1" --start 2030-12-02 --end 2030-12-15
```

Same flags as create; pass both `--start` and `--end` to change dates. To clear the dates: `echo '{"start_date":null,"end_date":null}' | plane-cli --json cycles update --project ENG --cycle "Sprint 1" --body -`.

### delete / archive / unarchive

```bash
plane-cli --json cycles delete --project ENG --cycle "Sprint 1"      # data: null
plane-cli --json cycles archive --project ENG --cycle "Sprint 1"     # data: { "id", "archived": true }
plane-cli --json cycles unarchive --project ENG --cycle "Sprint 1"   # data: { "id", "archived": false }
```

`delete` has no confirmation flag, the same as `modules delete`. `delete` and `archive` are refused by a `[deletes]` policy rule for members not listed; `unarchive` is not. Only completed cycles can be archived (`400 Only completed cycles can be archived`). Archived cycles drop out of `cycles list` and show in `cycles list --archived`; `cycles items list` still works on them.

### items list / add / remove

```bash
plane-cli --json cycles items list --project ENG --cycle "Sprint 1"
plane-cli --json cycles items add --project ENG --cycle "Sprint 1" --work-items ENG-12,ENG-13
plane-cli --json cycles items remove --project ENG --cycle "Sprint 1" --work-items ENG-12,ENG-13
```

`--work-items` takes composite ids or UUIDs, comma-separated (`--work-item` is accepted with a warning). Composite ids must belong to `--project`; all are resolved before anything is written.

**Response `data`:** `items list` returns paginated work item objects. `items add` returns Plane's cycle-issue rows (`{ "id", "issue", "cycle", … }`) for every item now in the cycle, not only the ones just added. `items remove` returns `{ "cycle": "<UUID>", "removed": [{ "input": "ENG-12", "id": "<UUID>" }, …] }`; if one fails, the error keeps that failure's `error.code` and exit code, its message names the items already removed, and `error.details` is `{ "removed": [{ "input", "id" }, …], "failed": "<input>", "cause": <Plane error body, if any> }`. Repeated ids are sent once. Removing an item that is not in the cycle is `404 not_found`.

Worked example ("Sprint 12" held ENG-1 to 7 but should hold only 1, 6 and 7):

```bash
plane-cli --json cycles items remove --project ENG --cycle "Sprint 12" \
  --work-items ENG-2,ENG-3,ENG-4,ENG-5
plane-cli --json cycles items list --project ENG --cycle "Sprint 12"
```

### transfer

```bash
plane-cli --json cycles transfer --project ENG --cycle "Sprint 1" --to "Sprint 2"
```

**Response `data`:** `{ "from": "<UUID>", "to": "<UUID>", "message": "Success" }`.

### How CE 1.4.2 behaves

1. **Adding an item that is already in another cycle moves it.** Plane allows one cycle per work item; `items add` returns 200 and the item leaves its old cycle with no warning, including when the old cycle is completed. Check `cycles items list` on the old cycle if that matters. Items cannot be added to a completed cycle (`400 CYCLE_COMPLETED`).
2. **`transfer` moves only unfinished items, and only out of a completed cycle.** Items whose state is in the `completed` or `cancelled` group stay in the source cycle; everything else moves to `--to`. If the source cycle's end date has not passed, Plane refuses with `400 The old cycle is not completed yet`.
3. **Dates are both-or-neither and stored in the project's time zone.** Create rejects one date without the other (`Both start date and end date are either required or are to be null`). Plane keeps only the date part of each value and stores the start as 00:00:01 and the end as 23:59 in the project's time zone: in a project set to `Etc/GMT-10` (UTC+10), `--start 2030-12-01 --end 2030-12-05` comes back as `2030-11-30T14:00:01Z` / `2030-12-05T13:59:00Z`, so `2030-12-24T13:59:00Z` is the end of the 24th there. The cycle's own `timezone` field (`UTC` on every cycle checked) does not change this, so plane-cli has no time-zone flag. A `PATCH` with only one date is accepted but stores the raw value with no conversion (midnight UTC), which is why plane-cli requires both. Do not feed a timestamp from output back in as a date: `2030-11-30T14:00:01Z` would be read as 30 November, so plane-cli rejects anything but `YYYY-MM-DD`. Plane does not stop cycles overlapping.
