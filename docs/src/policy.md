# Write policy

A project can have a policy file. plane-cli checks it before a write and refuses a write that breaks a rule, with the fix in the error. This gives agents a hard limit instead of a written instruction.

```bash
plane-cli config policy init --project ENG --owner alice --require priority
plane-cli config policy show --project ENG
plane-cli work-items update --work-item ENG-42 --state Done --dry-run --json
```

## Where policies live

- Global: `$XDG_CONFIG_HOME/plane-cli/policy/<PROJECT>.toml` (default `~/.config/plane-cli/policy/`).
- Repository: `.plane-cli/policy/<PROJECT>.toml`, written with `config policy init --local`.

Repository files are merged over the global file and can only make it stricter. A repository file is skipped with a warning if it, or its directory, is a symlink, is owned by someone else, or is writable by group or others.

## Rules

A section that is absent is not enforced.

| Rule | Section | Refuses |
|------|---------|---------|
| `search_before_create` | `[search_before_create]` | A new item whose name closely matches an open item, unless `--allow-duplicate` is passed |
| `description_shape` | `[description]` | A description missing the required sections, or a single plain paragraph |
| `required_fields` | `[required_fields]` | A create missing a listed field |
| `allowed_labels` | `[labels]` | Labels outside the allowed set |
| `comment_on_change` | `[comment_on_change]` | A state, assignee or priority change without `--reason` |
| `state_permissions` | `[[state_rules]]` | Moving an item into a listed state by a member who is not listed |
| `no_deletes` | `[deletes]` | Deletes and archives by a member who is not listed |

The caller is the `--as` member, or otherwise the owner of the API key.

## Example

```toml
version = 1

[search_before_create]
threshold = 0.85

[required_fields]
on_create = ["priority"]

[comment_on_change]
fields = ["state", "assignees", "priority"]

[[state_rules]]
states = ["Done", "Cancelled"]
members = ["alice"]

[deletes]
members = ["alice"]
```

## Refusals

A refused write returns `ok: false`, `error.code: "policy_violation"` and exit code 2. `error.details.violations` lists every broken rule with its `fix`. Nothing is written. `--dry-run` runs the same checks and reports `data.allowed` without writing.

Comment and link creates, relations, and module and cycle membership changes are not checked.

The [CRUD reference](https://github.com/pseudovar/plane-cli/blob/main/skills/plane-cli/CRUD-REFERENCE.md) has the merge rules and full response shapes.
