# Inbox

`plane-cli inbox` answers one question for an agent or a person: what should I work on now? It returns open work items assigned to the acting member, in small batches.

```bash
plane-cli --json --as ci-bot inbox
```

It returns two lists, each ordered by priority and then oldest first:

- `data.todo`: new work in the `unstarted` state group (default limit 3).
- `data.blocked_reassess`: items in the Blocked state to look at again (default limit 1).

In Progress items are left out, because a run that already picked an item up should not pick it up again. Backlog items are included only with `--include-backlog`. Done and Cancelled items are never returned.

## Using it from a scheduler

- `data.empty: true` means there is nothing to do. It is `ok: true` and exit 0, so a scheduled run can stop here.
- `data.partial: true` means a project could not be read. It is listed in `data.skipped_projects`. Partial is never empty.
- `data.counts` says how many more items are waiting.

## Per-project rules (`--handlers`)

Some projects use different workflow states. A handlers file lists which state names count as new work for each project:

```json
{
  "projects": {
    "OPS": { "states": ["Backlog", "Needs Triage"], "blocked_state": "On Hold" },
    "WEB": { "states": ["Todo"] }
  }
}
```

```bash
plane-cli --json --as ci-bot inbox --handlers ./handlers.json
```

- `states` replaces the default `todo` rule for that project.
- `blocked_state` (optional) names that project's Blocked state.
- A project that is not listed uses the defaults.

Each returned item also has a `handler_skill` field, a suggested name of the form `plane-<project>-inbox`. plane-cli ships no such handlers and does not act on the field, so you can ignore it or use it to route items to your own scripts.

## Limits

The inbox is based on assignment only. Plane Community Edition 1.4.2 has no public notifications API, so @mentions are not included. Assignee filtering happens in the client, so each project's work items are listed, up to 5,000 per project.
