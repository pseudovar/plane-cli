# Terminal UI (TUI) — current behavior

Human-oriented full-screen interface. **Automation and agents** should use `plane-cli --json` and [JSON output and exit codes](agent-contract.md); the TUI does not print JSON to stdout for normal use.

**Strict catalog:** The non-interactive CLI can enforce `--strict-catalog` (resolve state/label names to UUIDs before mutating work items). The TUI does **not** apply that same normalization path on PATCH bodies; it uses catalogs for pickers and labels, not as a pre-flight gate. To change that, treat it as a product change and a separate feature issue.

**Entry point:** `plane-cli` with the TUI subcommand from `plane-cli/src/dispatch/mod.rs` → `crate::tui::run_tui` (`plane-cli/src/tui/run.rs`).

---

## Module layout (source)

| File | Role |
|------|------|
| `plane-cli/src/tui/mod.rs` | Wires submodules, exports `run_tui` |
| `plane-cli/src/tui/project_open.rs` | Shared “open project work list” API sequence (states, labels, first page) for the worker |
| `plane-cli/src/tui/run.rs` | Event loop, key routing, `try_recv` UI updates |
| `plane-cli/src/tui/app.rs` | `App`, `Screen`, table state, modal/chart flags |
| `plane-cli/src/tui/msg.rs` | `UiMsg` / `Job` — worker ↔ UI contract |
| `plane-cli/src/tui/worker.rs` | Single consumer thread; serialized API queue |
| `plane-cli/src/tui/view.rs` | All `Frame` drawing (tables, preview, modals, footer) |
| `plane-cli/src/tui/format.rs` | Preview text, state/priority/assignee strings, merge/remove helpers |
| `plane-cli/src/tui/modal.rs` | Mutation modals (state, priority, assign, title, description, delete confirm) |
| `plane-cli/src/tui/charts.rs` | State distribution + activity bucketing (best-effort) |
| `plane-cli/src/tui/help_text.rs` | `?` overlay copy (contextual + global) |
| `plane-cli/src/tui/theme.rs` | Styles; respects `NO_COLOR` |
| `plane-cli/src/tui/wizards.rs` | Config init / skill installer alternate-screen flows |

Input routing lives in **`run.rs`** (not a separate `keys.rs`). Preview formatting lives mainly in **`format.rs`** + **`view.rs`** (not a separate `preview.rs`).

---

## Concurrency and errors

- **Serialized worker:** One background thread owns `PlaneApi` and processes **`Job`** messages in order. Multiple UI-driven requests queue up; there is **no job-generation ID or cancellation** — last-submitted work completes in sequence.
- **Error policy (footer):** Failed API work surfaces as `error: …` in the status line. **Previously loaded lists and selection are kept** unless the user navigates away or triggers a reload. The footer reminds you to **retry the same action** after a failure.

Background threads (update check, skill check) also send on the same `UiMsg` channel; their errors follow the same pattern when applicable.

---

## Behavior summary

| Area | Behavior |
|------|------------|
| Audience | Rich TUI for humans; automation uses **`--json`** |
| Write parity | TUI mutations align with CLI PATCH fields where applicable |
| Search | **`/`** on work items; project-scoped default; **`Ctrl-G`** toggles workspace scope; **`Esc`** clears search or restores browse snapshot |
| Mutation UX | **Shift+S** / **Shift+P** / **Shift+A** (state, priority, assignee); **e** / **w** (title, description); **Shift+D** → delete confirm (**y** / **n**) |
| Charts | **v** — state distribution strip from **loaded pages only**. **Shift+G** — activity sparkline from **`list_activities`**, labeled **best-effort** |
| Layout | Master–detail when the terminal is wide/tall enough; otherwise table-only (see thresholds in `view.rs`: `PREVIEW_MIN_WIDTH` / `PREVIEW_MIN_HEIGHT`) |
| Help | **?** contextual help; **?** again or **a** → global overlay |

---

## Screens and behavior

### Projects

Select a project, **Enter** loads work items (**states** + **labels** catalogs for that project).

### Work items

- **Table:** `#`, title, state (from catalog / JSON), priority, assignee summary.
- **Preview pane:** Title, state, assignee, labels, description excerpt; **module** line when the JSON includes `issue_module`, `modules`, or `module` (see `format::modules_preview_hint`).
- **Search:** **`/`** — draft query, **Enter** runs search through the worker.
- **Charts:** **v** toggles distribution strip; **Shift+G** loads activity trend for the **focused row** (shown on this screen).

### Modules / module issues

**m** from projects or work items loads modules; **Enter** opens issues for a module. Same mutation keys as work items when a row is focused.

### Detail

Full issue + comments; **c** starts comment draft; **[** / **]** or PgUp/PgDn scroll comments; **b** back. Mutations use the same Shift+* / **e** / **w** / **Shift+D** bindings.

---

## Assignees (API + TUI limits)

- **API:** Work items expose **`assignees`** as an array of member objects (e.g. `id`, `display_name`, …) on responses the CLI/TUI already loads.
- **Strict CLI normalization** (`plane-core`): PATCH **`assignees`** is an array of **user UUID strings** — see [JSON output and exit codes](agent-contract.md).
- **TUI assignee picker:** Options are built from **assignees seen on currently loaded work-item pages**, plus **clear**. To assign someone **not** in that set, use **`plane-cli work-items update --assignee <uuid>`** (repeatable) or **`--body`** with a full `assignees` array.

---

## Activity chart (Shift+G)

- Worker calls **`list_activities`** for the focused issue with pagination (bounded loop over cursor pages).
- Bucketing groups by **calendar day** of `created_at` (see `charts::activity_buckets_from_results`). Label includes counts and **best-effort** wording — not a guaranteed product analytics view.
- If the API returns no activities, the sparkline may be empty or trivial; distribution (**v**) is unchanged.

---

## Key reference (summary)

Full lists are in **`?`** inside the TUI (`help_text.rs`). Globally: **q** quit, **r** refresh project list (from projects), **s** env wizard (same as `config init`, including optional **`PLANE_CLI_ANON_TELEMETRY`** opt-in), **i** skill installer.

---

## Related documentation

- [JSON output and exit codes](agent-contract.md) — env, `--json`, CLI flags including **`--assignee`**, completions
- `skills/plane-cli/SKILL.md` — agent skill install for IDE assistants
