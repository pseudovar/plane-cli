# plane-cli agent workflows

Behavioral rules when mutating Plane on behalf of users. For exact CLI/JSON shapes see [CRUD-REFERENCE.md](CRUD-REFERENCE.md).

Strength key: **MUST** = non-negotiable for agents · **SHOULD** = default unless user says otherwise · **MAY** = org-specific (override in your agent rules or team docs).

---

## MUST — agent safety

1. **Read before write** — `get` or `list` the target resource (and catalogs via `complete`) before any mutation.
2. **Never delete or close** work items, projects, modules, cycles, labels, states, intake items, attachments, comments, or links, and never remove project members or send invitations, unless the user explicitly asked.
3. **Confirm after mutate** — re-fetch the resource and verify the expected fields changed.
4. **Audit meaningful changes** — on state, assignee, or priority changes, add a comment explaining what changed and why (unless the user said not to). `work-items update --reason "<why>"` posts it in the same run; a project policy can make it mandatory.
5. **Never guess identifiers** — resolve states/labels with `complete` + `--strict-catalog`; never invent subcommands or API paths.
6. **Use composite ids in summaries** — report `ENG-123`, not bare UUIDs, when talking to the user.
7. **Always `--json`** — parse `ok` and `error.code`; never rely on stderr prose alone.
8. **Minimal patches** — send only fields that change; do not overwrite unrelated fields via large `--body` blobs.

---

## SHOULD — agile hygiene

### Comments on updates

Add a comment when you change anything a teammate would notice:

- State transitions (e.g. Todo → In Progress → Done)
- Reassignments
- Priority changes
- Scope changes (also update `--description` when material)

**Template** (drop the `[agent]` prefix when you call with `--as <your-member>`: Plane already shows you as the author):

```text
[agent] <action>: <what changed>. <why / next step>.
```

Example `--comment-html`:

```html
<p>[agent] Handoff: moved to <strong>In Review</strong>, assigned to Jane. Ready for QA on staging.</p>
```

### Handoffs (state + assignee + comment)

When moving work to someone else, do all three in one session:

1. `work-items get` — capture current state and assignees.
2. `complete states` — confirm target state name exists.
3. `work-items update` — `--state` and `--assignee <next-owner-uuid>`.
4. `work-items comments create` — explain the handoff.
5. `work-items get` — confirm.

```bash
plane-cli --json --strict-catalog work-items update --project ENG --work-item ENG-42 \
  --state "In Review" --assignee 550e8400-e29b-41d4-a716-446655440001

plane-cli --json work-items comments create --project ENG --work-item ENG-42 \
  --comment-html "<p>[agent] Handoff to review — implementation complete, ready for QA.</p>"
```

### Assign the next owner

When moving to **In Progress**, **In Review**, or **Done**, set `--assignee` to whoever should act next:

- **In Progress** → implementer
- **In Review** → reviewer
- **Done** → often clear assignees or leave as completer (team preference)

If the next owner is unknown, **SHOULD** comment that the item is unassigned and ready for pickup rather than guessing a UUID.

### Blocked / waiting

When work is blocked:

1. Move to an appropriate state if the board has one (resolve via `complete states`).
2. Comment with the blocker and link to the blocking item if it exists.
3. Optionally `relations create --relation-type blocked_by --issues <blocker-uuid>`.

### Closing work

Before marking Done:

- Comment with outcome (what shipped, PR link, caveats).
- Add `work-items links create` for PR/docs when relevant.
- Only then `--state Done` (or your team's equivalent).

### Creating work

1. **Search first** — `work-items search --query "…"` to avoid duplicates.
2. Create with clear `--name`, sensible `--state`, `--assignee` if known.
3. Add labels via `--body -` when the board uses them (see CRUD-REFERENCE).
4. Comment with context if the title alone is not enough.

### Bulk changes

1. List affected items first; show the user what will change.
2. Update one item, verify, then proceed with the rest (or stop on first error).
3. Never assume all items share the same current state.

### Relations and links

- Prefer `relations create` for blocks/duplicates/relates over mentioning ids only in comments.
- Prefer `links create` for URLs (PRs, docs, dashboards) over bare URLs in comments alone.

### Modules / epics

When work belongs to a module, use `modules issues add` — not just a comment mentioning the module name.

The same goes for cycles: `cycles items add|remove --work-items ENG-12,ENG-13`. A work item can be in one cycle only, so adding it to a cycle takes it out of its current one.

---

## MAY — org-specific overrides

Some of these rules can be enforced per project by plane-cli instead of left to instructions: `plane-cli config policy init --project P` writes a policy covering search before create, description sections, required fields and labels, a `--reason` comment on state/assignee/priority changes, who may set which states, and who may delete. With a policy in place, a write that breaks a rule is refused with `error.code: "policy_violation"` and the fix (see CRUD-REFERENCE.md, "config policy"). Check `projects context` → `policy` before writing.

Document the rest in your agent rules or a team doc if they differ from defaults:

| Policy | Example override |
|--------|------------------|
| Who gets assigned on handoff | Round-robin, on-call rotation, sprint owner, self-assign in Review |
| Required fields per state | "In Progress must have assignee" |
| Label conventions | `bug`, `tech-debt`, sprint labels |
| Comment format | `@mention` style, ticket templates |
| When to clear assignees | On Done vs keep completer |

**Default when unclear:** ask the user who should be assigned rather than picking randomly.

---

## Standard execution checklist

Use this sequence for any mutation task:

```
[ ] auth whoami — connectivity OK
[ ] Resolve project (projects list / complete projects)
[ ] Read target (get / list)
[ ] Resolve catalogs if needed (complete states/labels)
[ ] Read CRUD-REFERENCE.md for this command (or --help)
[ ] Mutate with --json
[ ] Comment if meaningful change (WORKFLOWS MUST #4)
[ ] Re-fetch and confirm
[ ] Summarize to user with composite id (ENG-123)
```

---

## Common agent mistakes (workflow)

| Mistake | Fix |
|---------|-----|
| Silent state change | Add comment + confirm with get |
| Update without checking current state | get first; skip if already in target state |
| Assign wrong person | Ask user or use UUID from prior comment/assignee list |
| Duplicate tickets | search before create |
| Inline JSON in `--body` | Use `--body -` + stdin or `--body file.json` |
| Invent `work-items list-states` | `complete states --project ENG` |
