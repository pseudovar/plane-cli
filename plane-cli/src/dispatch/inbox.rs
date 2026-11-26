//! `plane-cli inbox`: fetch the work assigned to the acting member.
//!
//! Work items assigned to the acting member (`--as`, else the key's own `users/me`), across
//! the projects that member belongs to. `todo` is new work: the `unstarted` state group (Todo)
//! only (an item moves to In Progress when the agent acts, so it is not fetched
//! again); `--include-backlog` adds the `backlog` group, and a
//! `--handlers` file lists the eligible state names per project instead. `blocked_reassess`
//! is items in the Blocked state (`--blocked-state`, or `blocked_state` in the handlers file),
//! never filtered by the handlers' state list. Terminal groups (`completed`, `cancelled`) are
//! never returned. Each list is ordered by priority (urgent first), then oldest `created_at`,
//! and capped (3 and 1 by default: the batch cap). An empty inbox is `ok: true`, `empty: true`;
//! a scan that skipped a project the member cannot read is `partial: true` and never empty.
//!
//! CE 1.4.2 has no server-side assignee filter and no notifications endpoint, so this lists
//! each project's work items (`expand=state`) and filters here; mentions are not covered.
//! Checked against Plane Community Edition 1.4.2.

use crate::output;
use clap::ValueHint;
use plane_core::{CoreError, PageParams, PlaneApi, ProjectEntry};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default batch cap: 3 Todo items plus 1 Blocked reassess per turn.
pub const DEFAULT_TODO_LIMIT: usize = 3;
pub const DEFAULT_BLOCKED_LIMIT: usize = 1;

#[derive(clap::Args, Clone, Debug)]
pub struct InboxArgs {
    #[arg(
        long,
        value_name = "PROJECT",
        value_hint = ValueHint::Other,
        help = "Only this project (identifier or UUID); default: every project the member belongs to"
    )]
    pub project: Option<String>,
    #[arg(
        long,
        value_name = "N",
        default_value_t = DEFAULT_TODO_LIMIT,
        help = "Most items returned in `todo` (the batch cap)"
    )]
    pub limit: usize,
    #[arg(
        long,
        value_name = "N",
        default_value_t = DEFAULT_BLOCKED_LIMIT,
        help = "Most items returned in `blocked_reassess` (0 skips the Blocked pass)"
    )]
    pub blocked_limit: usize,
    #[arg(
        long,
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "JSON file: per-project `states` eligible for `todo` and optional `blocked_state` ({\"projects\":{\"OPS\":{\"states\":[...]}}}); projects not listed use the defaults"
    )]
    pub handlers: Option<PathBuf>,
    #[arg(
        long,
        help = "Also treat the `backlog` state group as new work (default: only `unstarted`, e.g. Todo)"
    )]
    pub include_backlog: bool,
    #[arg(
        long,
        value_name = "NAME",
        default_value = DEFAULT_BLOCKED_STATE,
        help = "State name of the Blocked pass (case-insensitive); a handlers-file `blocked_state` overrides it per project"
    )]
    pub blocked_state: String,
}

pub const DEFAULT_BLOCKED_STATE: &str = "Blocked";

/// One project's entry in the handlers file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HandlerEntry {
    /// Lower-case state names eligible for `todo`; `None` = the default groups.
    pub states: Option<Vec<String>>,
    pub blocked_state: Option<String>,
}

/// Handlers file by upper-case project identifier.
pub type Handlers = HashMap<String, HandlerEntry>;

pub fn load_handlers(path: &Path) -> Result<Handlers, CoreError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CoreError::usage_code(
            "handlers_file_unreadable",
            format!("cannot read --handlers {}: {e}", path.display()),
        )
    })?;
    let v: Value = serde_json::from_str(&text).map_err(|e| {
        CoreError::usage_code(
            "handlers_invalid_json",
            format!("--handlers {} is not valid JSON: {e}", path.display()),
        )
    })?;
    parse_handlers(&v)
}

pub fn parse_handlers(v: &Value) -> Result<Handlers, CoreError> {
    let projects = v.get("projects").and_then(Value::as_object).ok_or_else(|| {
        CoreError::usage_code(
            "handlers_invalid_json",
            "--handlers file needs a `projects` object, e.g. {\"projects\":{\"OPS\":{\"states\":[\"Backlog\"]}}}",
        )
    })?;
    let mut out = Handlers::new();
    for (ident, entry) in projects {
        let states = entry.get("states").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_lowercase())
                .collect::<Vec<_>>()
        });
        let blocked_state = entry
            .get("blocked_state")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        out.insert(
            ident.to_uppercase(),
            HandlerEntry {
                states,
                blocked_state,
            },
        );
    }
    Ok(out)
}

/// Priority rank for ordering: urgent first, `none` / unknown last.
pub fn priority_rank(p: Option<&str>) -> u8 {
    match p.unwrap_or("none") {
        "urgent" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

pub fn is_terminal_group(group: Option<&str>) -> bool {
    matches!(group, Some("completed") | Some("cancelled"))
}

/// How one project's items are classified.
#[derive(Debug, Clone)]
pub struct ScanRules<'a> {
    /// Handlers-file `states` for this project (lower-case), eligible for `todo`.
    pub eligible: Option<&'a [String]>,
    pub blocked_state: &'a str,
    pub include_backlog: bool,
}

impl Default for ScanRules<'_> {
    fn default() -> Self {
        ScanRules {
            eligible: None,
            blocked_state: DEFAULT_BLOCKED_STATE,
            include_backlog: false,
        }
    }
}

/// `assignees` is a list of UUID strings, or of objects with `id` when expanded.
pub fn assigned_to(item: &Value, member_id: &str) -> bool {
    item.get("assignees")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter().any(|a| {
                a.as_str()
                    .or_else(|| a.get("id").and_then(Value::as_str))
                    .is_some_and(|id| id.eq_ignore_ascii_case(member_id))
            })
        })
        .unwrap_or(false)
}

/// A state as seen on a listed work item: expanded object, or a bare UUID looked up in `states`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemState {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
}

pub fn item_state(item: &Value, states: &HashMap<String, ItemState>) -> Option<ItemState> {
    match item.get("state") {
        Some(Value::Object(o)) => Some(ItemState {
            id: o
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            name: o
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            group: o.get("group").and_then(Value::as_str).map(String::from),
        }),
        Some(Value::String(id)) => states.get(id).cloned(),
        _ => None,
    }
}

/// One inbox entry, already reduced to the fields a caller needs.
#[derive(Debug, Clone)]
pub struct InboxItem {
    pub value: Value,
    pub rank: u8,
    pub created_at: String,
    pub sequence: i64,
    pub blocked: bool,
}

/// Outcome of filtering one project's list.
#[derive(Debug, Default)]
pub struct ProjectScan {
    pub items: Vec<InboxItem>,
    /// Open assigned items (any non-terminal state), before the new-work rules.
    pub assigned_open: usize,
    pub skipped_by_handlers: usize,
    /// Open items in the `started` group (In Progress): already being worked on.
    pub skipped_started: usize,
    /// Open items in the `backlog` group without `--include-backlog`.
    pub skipped_backlog: usize,
    /// Open items in other groups (e.g. `triage`).
    pub skipped_other: usize,
}

/// Filter one project's work items for `member_id` under `rules`.
pub fn scan_project(
    project: &ProjectEntry,
    items: &[Value],
    member_id: &str,
    states: &HashMap<String, ItemState>,
    rules: &ScanRules,
) -> ProjectScan {
    let mut scan = ProjectScan::default();
    for item in items {
        if !assigned_to(item, member_id) {
            continue;
        }
        if item.get("archived_at").is_some_and(|v| !v.is_null()) {
            continue;
        }
        let Some(state) = item_state(item, states) else {
            continue;
        };
        if is_terminal_group(state.group.as_deref()) {
            continue;
        }
        scan.assigned_open += 1;
        let blocked = state
            .name
            .trim()
            .eq_ignore_ascii_case(rules.blocked_state.trim());
        if !blocked {
            match (rules.eligible, state.group.as_deref()) {
                (Some(eligible), _) => {
                    if !eligible.contains(&state.name.trim().to_lowercase()) {
                        scan.skipped_by_handlers += 1;
                        continue;
                    }
                }
                (None, Some("unstarted")) => {}
                (None, Some("backlog")) if rules.include_backlog => {}
                (None, Some("backlog")) => {
                    scan.skipped_backlog += 1;
                    continue;
                }
                (None, Some("started")) => {
                    scan.skipped_started += 1;
                    continue;
                }
                (None, _) => {
                    scan.skipped_other += 1;
                    continue;
                }
            }
        }
        let priority = item.get("priority").and_then(Value::as_str);
        let sequence = item.get("sequence_id").and_then(Value::as_i64).unwrap_or(0);
        let created_at = item
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let composite = format!("{}-{}", project.identifier, sequence);
        let value = json!({
            "id": item.get("id").cloned().unwrap_or(Value::Null),
            "composite_id": composite,
            "name": item.get("name").cloned().unwrap_or(Value::Null),
            "project": project.identifier,
            "project_id": project.id.to_string(),
            "state": {"id": state.id, "name": state.name, "group": state.group},
            "priority": priority.unwrap_or("none"),
            "created_at": created_at,
            "updated_at": item.get("updated_at").cloned().unwrap_or(Value::Null),
            "target_date": item.get("target_date").cloned().unwrap_or(Value::Null),
            "handler_skill": format!("plane-{}-inbox", project.identifier.to_lowercase()),
        });
        scan.items.push(InboxItem {
            value,
            rank: priority_rank(priority),
            created_at,
            sequence,
            blocked,
        });
    }
    scan
}

/// Priority, then oldest `created_at`, then project and sequence for a stable order.
pub fn sort_items(items: &mut [InboxItem]) {
    items.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| {
                a.value["project"]
                    .as_str()
                    .cmp(&b.value["project"].as_str())
            })
            .then_with(|| a.sequence.cmp(&b.sequence))
    });
}

/// Split into `todo` and `blocked_reassess`, order each and apply the caps.
pub fn build_batch(
    mut items: Vec<InboxItem>,
    todo_limit: usize,
    blocked_limit: usize,
) -> (Vec<Value>, Vec<Value>, usize, usize) {
    sort_items(&mut items);
    let (blocked, todo): (Vec<_>, Vec<_>) = items.into_iter().partition(|i| i.blocked);
    let (todo_total, blocked_total) = (todo.len(), blocked.len());
    let todo = todo.into_iter().take(todo_limit).map(|i| i.value).collect();
    let blocked = blocked
        .into_iter()
        .take(blocked_limit)
        .map(|i| i.value)
        .collect();
    (todo, blocked, todo_total, blocked_total)
}

/// Projects to scan: the one named by `--project`, else every unarchived project the
/// member belongs to (`is_member` from the projects list).
fn projects_to_scan(
    api: &mut PlaneApi,
    args: &InboxArgs,
    catalog: &[ProjectEntry],
    refresh: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<ProjectEntry>, CoreError> {
    if let Some(p) = &args.project {
        let pid = super::helpers::resolve_project_auto_refresh(api, p, catalog, refresh, warnings)?;
        let entry = api
            .fetch_projects_catalog(false)?
            .into_iter()
            .find(|e| e.id == pid)
            .unwrap_or(ProjectEntry {
                id: pid,
                name: String::new(),
                identifier: p.to_uppercase(),
            });
        return Ok(vec![entry]);
    }
    let (v, _, _) = api.list_projects_public(&PageParams::all())?;
    let rows = v
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let all = plane_core::projects_from_value(&v)?;
    Ok(all
        .into_iter()
        .zip(rows.iter())
        .filter(|(_, row)| {
            row.get("is_member")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                && row.get("archived_at").is_none_or(Value::is_null)
        })
        .map(|(e, _)| e)
        .collect())
}

pub fn run(
    args: &InboxArgs,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
) -> Result<(), CoreError> {
    let handlers = match &args.handlers {
        Some(p) => Some(load_handlers(p)?),
        None => None,
    };
    let me = api.whoami()?;
    let member_id = me
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| CoreError::UnexpectedResponse("users/me has no id".into()))?
        .to_string();

    let mut warnings = Vec::new();
    let projects = projects_to_scan(api, args, catalog, refresh, &mut warnings)?;
    let query = vec![("expand".to_string(), "state".to_string())];
    let mut items = Vec::new();
    let mut totals = ProjectScan::default();
    let mut scanned = Vec::new();
    let mut skipped_projects = Vec::new();
    for project in &projects {
        let (v, _, truncated) =
            match api.list_work_items_query(&project.id, &query, &PageParams::all()) {
                Ok(r) => r,
                // A project the member cannot read is skipped, not fatal for the whole inbox.
                Err(
                    e @ CoreError::Http {
                        status: 403 | 404, ..
                    },
                ) if args.project.is_none() => {
                    warnings.push(format!(
                        "skipped project {}: {}; the inbox is partial",
                        project.identifier,
                        e.error_code()
                    ));
                    skipped_projects
                        .push(json!({"project": project.identifier, "code": e.error_code()}));
                    continue;
                }
                Err(e) => return Err(e),
            };
        if truncated {
            warnings.push(format!(
                "project {} has more work items than the list cap; some assigned items may be missing",
                project.identifier
            ));
        }
        let rows = v
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // `expand=state` is honoured by CE 1.4.2; fall back to the states catalog otherwise.
        let mut states = HashMap::new();
        if rows
            .iter()
            .any(|r| r.get("state").is_some_and(Value::is_string))
        {
            for s in api.fetch_states_catalog(&project.id, refresh)? {
                states.insert(
                    s.id.to_string(),
                    ItemState {
                        id: s.id.to_string(),
                        name: s.name,
                        group: s.group,
                    },
                );
            }
        }
        let entry = handlers
            .as_ref()
            .and_then(|h| h.get(&project.identifier.to_uppercase()));
        let rules = ScanRules {
            eligible: entry.and_then(|e| e.states.as_deref()),
            blocked_state: entry
                .and_then(|e| e.blocked_state.as_deref())
                .unwrap_or(&args.blocked_state),
            include_backlog: args.include_backlog,
        };
        let scan = scan_project(project, &rows, &member_id, &states, &rules);
        totals.assigned_open += scan.assigned_open;
        totals.skipped_by_handlers += scan.skipped_by_handlers;
        totals.skipped_started += scan.skipped_started;
        totals.skipped_backlog += scan.skipped_backlog;
        totals.skipped_other += scan.skipped_other;
        items.extend(scan.items);
        scanned.push(project.identifier.clone());
    }

    let (todo, blocked, todo_total, blocked_total) =
        build_batch(items, args.limit, args.blocked_limit);
    let partial = !skipped_projects.is_empty();
    let data = json!({
        // A partial scan is never "empty": the skipped projects may hold work.
        "empty": todo.is_empty() && blocked.is_empty() && !partial,
        "partial": partial,
        "skipped_projects": skipped_projects,
        "member": {
            "id": member_id,
            "display_name": me.get("display_name").cloned().unwrap_or(Value::Null),
            "acting_as": crate::identity::current_member(),
        },
        "todo": todo,
        "blocked_reassess": blocked,
        "counts": {
            "assigned_open": totals.assigned_open,
            "todo": todo_total,
            "blocked": blocked_total,
            "skipped_in_progress": totals.skipped_started,
            "skipped_backlog": totals.skipped_backlog,
            "skipped_other": totals.skipped_other,
            "skipped_by_handlers": totals.skipped_by_handlers,
        },
        "limits": {"todo": args.limit, "blocked": args.blocked_limit},
        "rules": {
            "todo_groups": if args.include_backlog { json!(["unstarted", "backlog"]) } else { json!(["unstarted"]) },
            "blocked_state": args.blocked_state,
            "handlers": args.handlers.as_ref().map(|p| p.display().to_string()),
        },
        "projects_scanned": scanned,
    });
    output::emit_success_with_warnings(json_mode, data, None, false, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    const ME: &str = "7b871681-a7ab-4470-b642-88305803b29b";

    fn project() -> ProjectEntry {
        ProjectEntry {
            id: Uuid::nil(),
            name: "Test".into(),
            identifier: "WEB".into(),
        }
    }

    fn item(seq: i64, prio: &str, created: &str, state: (&str, &str), assignee: &str) -> Value {
        json!({
            "id": format!("00000000-0000-0000-0000-{seq:012}"),
            "name": format!("item {seq}"),
            "sequence_id": seq,
            "priority": prio,
            "created_at": created,
            "state": {"id": format!("s-{}", state.0), "name": state.0, "group": state.1},
            "assignees": [assignee],
        })
    }

    fn ids(v: &[Value]) -> Vec<&str> {
        v.iter()
            .map(|x| x["composite_id"].as_str().unwrap())
            .collect()
    }

    fn seqs(scan: &ProjectScan) -> Vec<i64> {
        scan.items.iter().map(|i| i.sequence).collect()
    }

    const T: &str = "2030-01-01T00:00:00Z";

    #[test]
    fn new_work_is_unstarted_only_by_default() {
        let rows = vec![
            item(1, "high", T, ("Todo", "unstarted"), ME),
            item(2, "high", T, ("Done", "completed"), ME),
            item(3, "high", T, ("Cancelled", "cancelled"), ME),
            item(4, "high", T, ("Todo", "unstarted"), "someone-else"),
            item(5, "low", T, ("In Progress", "started"), ME),
            item(6, "low", T, ("Backlog", "backlog"), ME),
        ];
        let scan = scan_project(
            &project(),
            &rows,
            ME,
            &HashMap::new(),
            &ScanRules::default(),
        );
        assert_eq!(seqs(&scan), vec![1]);
        assert_eq!(scan.assigned_open, 3);
        assert_eq!((scan.skipped_started, scan.skipped_backlog), (1, 1));
        let with_backlog = ScanRules {
            include_backlog: true,
            ..ScanRules::default()
        };
        let scan = scan_project(&project(), &rows, ME, &HashMap::new(), &with_backlog);
        assert_eq!(seqs(&scan), vec![1, 6]);
    }

    #[test]
    fn orders_by_priority_then_oldest_and_applies_caps() {
        let rows = vec![
            item(1, "low", "2030-01-01T00:00:00Z", ("Todo", "unstarted"), ME),
            item(
                2,
                "urgent",
                "2030-03-01T00:00:00Z",
                ("Todo", "unstarted"),
                ME,
            ),
            item(3, "high", "2030-02-01T00:00:00Z", ("Todo", "unstarted"), ME),
            item(
                4,
                "high",
                "2030-01-15T00:00:00Z",
                ("Ready", "unstarted"),
                ME,
            ),
            item(5, "none", "2025-01-01T00:00:00Z", ("Todo", "unstarted"), ME),
            item(
                6,
                "medium",
                "2030-01-01T00:00:00Z",
                ("Blocked", "started"),
                ME,
            ),
            item(
                7,
                "urgent",
                "2030-05-01T00:00:00Z",
                ("blocked", "unstarted"),
                ME,
            ),
        ];
        let scan = scan_project(
            &project(),
            &rows,
            ME,
            &HashMap::new(),
            &ScanRules::default(),
        );
        let (todo, blocked, todo_total, blocked_total) = build_batch(scan.items, 3, 1);
        assert_eq!(ids(&todo), vec!["WEB-2", "WEB-4", "WEB-3"]);
        assert_eq!(ids(&blocked), vec!["WEB-7"]);
        assert_eq!((todo_total, blocked_total), (5, 2));
    }

    #[test]
    fn blocked_limit_zero_skips_blocked_pass() {
        let rows = vec![item(6, "medium", T, ("Blocked", "started"), ME)];
        let scan = scan_project(
            &project(),
            &rows,
            ME,
            &HashMap::new(),
            &ScanRules::default(),
        );
        let (todo, blocked, _, blocked_total) = build_batch(scan.items, 3, 0);
        assert!(todo.is_empty() && blocked.is_empty());
        assert_eq!(blocked_total, 1);
    }

    #[test]
    fn handlers_states_apply_to_todo_only_never_to_the_blocked_pass() {
        let h = parse_handlers(&json!({
            "projects": {
                "web": {"states": ["Todo", "In Progress"]},
                "OPS": {"states": ["Backlog"], "blocked_state": "On Hold"}
            }
        }))
        .unwrap();
        let rows = vec![
            item(1, "high", T, ("Todo", "unstarted"), ME),
            item(2, "high", T, ("In Progress", "started"), ME),
            item(3, "high", T, ("Blocked", "started"), ME),
            item(4, "high", T, ("Review", "started"), ME),
        ];
        let web = h.get("WEB").unwrap();
        let rules = ScanRules {
            eligible: web.states.as_deref(),
            ..ScanRules::default()
        };
        let scan = scan_project(&project(), &rows, ME, &HashMap::new(), &rules);
        // "Blocked" is not in the handlers' states but still reaches the Blocked pass.
        assert_eq!(seqs(&scan), vec![1, 2, 3]);
        assert!(scan.items.iter().find(|i| i.sequence == 3).unwrap().blocked);
        assert_eq!(scan.skipped_by_handlers, 1);
        let ops = h.get("OPS").unwrap();
        assert_eq!(ops.blocked_state.as_deref(), Some("On Hold"));
        let rows = vec![
            item(5, "low", T, ("On Hold", "started"), ME),
            item(6, "low", T, ("Blocked", "started"), ME),
        ];
        let rules = ScanRules {
            eligible: ops.states.as_deref(),
            blocked_state: ops.blocked_state.as_deref().unwrap(),
            include_backlog: false,
        };
        let scan = scan_project(&project(), &rows, ME, &HashMap::new(), &rules);
        assert_eq!(seqs(&scan), vec![5]);
        assert!(scan.items[0].blocked);
        assert!(parse_handlers(&json!({"nope": 1})).is_err());
    }

    #[test]
    fn bare_state_ids_use_the_states_catalog_and_assignee_objects_match() {
        let mut states = HashMap::new();
        states.insert(
            "s1".to_string(),
            ItemState {
                id: "s1".into(),
                name: "Done".into(),
                group: Some("completed".into()),
            },
        );
        states.insert(
            "s2".to_string(),
            ItemState {
                id: "s2".into(),
                name: "Todo".into(),
                group: Some("unstarted".into()),
            },
        );
        let rows = vec![
            json!({"id": "a", "sequence_id": 1, "state": "s1", "assignees": [ME]}),
            json!({"id": "b", "sequence_id": 2, "state": "s2", "assignees": [{"id": ME}]}),
        ];
        let scan = scan_project(&project(), &rows, ME, &states, &ScanRules::default());
        assert_eq!(scan.items.len(), 1);
        assert_eq!(scan.items[0].value["state"]["name"], "Todo");
        assert_eq!(scan.items[0].value["handler_skill"], "plane-web-inbox");
    }
}
