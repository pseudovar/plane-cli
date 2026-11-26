//! Agent-oriented project snapshot (capabilities, grouped states, labels, hints).

use crate::catalog::{LabelEntry, ProjectEntry, StateEntry};
use crate::cycles::cycle_summaries;
use serde_json::{json, Value};

const STATE_BUCKETS: &[&str] = &["backlog", "unstarted", "started", "completed", "cancelled"];

/// JSON object for one workflow state (catalog row).
pub fn state_record(s: &StateEntry) -> Value {
    json!({
        "id": s.id.to_string(),
        "name": s.name,
        "group": s.group,
        "color": s.color,
        "sequence": s.sequence,
        "default": s.is_default,
    })
}

/// JSON object for one label (catalog row).
pub fn label_record(l: &LabelEntry) -> Value {
    json!({
        "id": l.id.to_string(),
        "name": l.name,
        "color": l.color,
        "description": l.description,
        "sort_order": l.sort_order,
    })
}

fn cmp_f64_then_str(
    a_seq: Option<f64>,
    a_name: &str,
    b_seq: Option<f64>,
    b_name: &str,
) -> std::cmp::Ordering {
    let a_key = a_seq.unwrap_or(f64::MAX);
    let b_key = b_seq.unwrap_or(f64::MAX);
    a_key
        .partial_cmp(&b_key)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            a_name
                .to_ascii_lowercase()
                .cmp(&b_name.to_ascii_lowercase())
        })
}

/// States sorted by `sequence` then name (stable for agents).
pub fn states_sorted(states: &[StateEntry]) -> Vec<&StateEntry> {
    let mut out: Vec<&StateEntry> = states.iter().collect();
    out.sort_by(|a, b| cmp_f64_then_str(a.sequence, &a.name, b.sequence, &b.name));
    out
}

fn labels_sorted(labels: &[LabelEntry]) -> Vec<&LabelEntry> {
    let mut out: Vec<&LabelEntry> = labels.iter().collect();
    out.sort_by(|a, b| cmp_f64_then_str(a.sort_order, &a.name, b.sort_order, &b.name));
    out
}

/// Workflow states grouped by Plane `group` (plus `triage` when present).
pub fn states_by_group(states: &[StateEntry]) -> Value {
    let mut buckets: std::collections::BTreeMap<String, Vec<Value>> = STATE_BUCKETS
        .iter()
        .map(|g| ((*g).to_string(), Vec::new()))
        .collect();
    buckets.insert("triage".into(), Vec::new());

    for s in states_sorted(states) {
        let key = s.group.as_deref().unwrap_or("backlog").to_ascii_lowercase();
        let slot = buckets.entry(key).or_default();
        slot.push(state_record(s));
    }

    let mut out = serde_json::Map::new();
    for g in STATE_BUCKETS {
        out.insert(
            (*g).to_string(),
            json!(buckets.remove(*g).unwrap_or_default()),
        );
    }
    out.insert(
        "triage".into(),
        json!(buckets.remove("triage").unwrap_or_default()),
    );
    for (k, v) in buckets {
        if !v.is_empty() {
            out.insert(k, json!(v));
        }
    }
    Value::Object(out)
}

/// Feature flags and estimate metadata from a project `GET` payload.
pub fn project_capabilities(project: &Value) -> Value {
    let flag = |k: &str| project.get(k).and_then(|x| x.as_bool());
    json!({
        "cycle_view": flag("cycle_view"),
        "module_view": flag("module_view"),
        "issue_views_view": flag("issue_views_view"),
        "page_view": flag("page_view"),
        "intake_view": flag("intake_view"),
        "is_time_tracking_enabled": flag("is_time_tracking_enabled"),
        "is_issue_type_enabled": flag("is_issue_type_enabled"),
        "default_state": project.get("default_state").cloned(),
        "archived": project.get("archived_at").map(|x| !x.is_null()).unwrap_or(false),
        "estimate": project.get("estimate").cloned(),
    })
}

/// `{ id, name }` rows from a paginated modules list response.
pub fn module_summaries(modules_list: &Value) -> Vec<Value> {
    let Some(arr) = modules_list.get("results").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    let mut rows: Vec<Value> = arr
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            let name = item
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            Some(json!({ "id": id, "name": name }))
        })
        .collect();
    rows.sort_by(|a, b| {
        let na = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let nb = b.get("name").and_then(|x| x.as_str()).unwrap_or("");
        na.cmp(nb)
    });
    rows
}

/// Short guidance strings for autonomous agents mutating this project.
pub fn agent_hints(
    capabilities: &Value,
    states: &[StateEntry],
    labels: &[LabelEntry],
) -> Vec<String> {
    let mut hints: Vec<String> = vec![
        "Preflight: `plane-cli --json projects context --id <PROJECT>` (or `complete states` / `complete labels`).".into(),
        "Workflow state on create/update: `--state <NAME>` with global `--strict-catalog` (names must match catalog).".into(),
        "Description: use `--description` (maps to API `description_html`); not a bare `description` field in `--body`.".into(),
        "Labels: no `--labels` flag; pass `labels` in `--body` JSON (name strings or UUIDs).".into(),
        "After `projects create`, rerun catalog commands with `--refresh` if identifier resolution fails.".into(),
    ];
    // An archived project reads as having no states; `states create` would not help.
    if states.is_empty() && capabilities.get("archived").and_then(|x| x.as_bool()) != Some(true) {
        hints.insert(
            0,
            "This project has no workflow states: work items created now get `state: null` and no board column. Add states first, e.g. `plane-cli --json states create --project <PROJECT> --name Todo --group unstarted --default` (groups: backlog, unstarted, started, completed, cancelled)."
                .into(),
        );
    }
    if labels.is_empty() {
        hints.push(
            "This project has no labels; omit `labels` in work-item bodies or add some with `plane-cli --json labels create --project <PROJECT> --name <NAME>`."
                .into(),
        );
    }
    if capabilities.get("module_view").and_then(|x| x.as_bool()) == Some(true) {
        hints.push(
            "Modules are addressed by UUID: `plane-cli --json complete modules --project <PROJECT>`."
                .into(),
        );
    }
    if capabilities.get("cycle_view").and_then(|x| x.as_bool()) == Some(true) {
        hints.push(
            "Cycles: `plane-cli --json cycles list --project <PROJECT>`; `--cycle` takes a UUID or exact name, `cycles items add|remove --work-items` takes PROJ-123 ids. Adding an item moves it out of any other cycle."
                .into(),
        );
    }
    if capabilities.get("intake_view").and_then(|x| x.as_bool()) == Some(true) {
        hints.push(
            "Intake is enabled on this project: `plane-cli --json intake list --project <PROJECT>`; accept or reject with `intake update --status accepted|rejected`."
                .into(),
        );
    }
    hints
}

/// `summary` section of `projects context` from a `GET projects/{id}/summary/` response:
/// `{ "counts": { issues, cycles, modules, members, states, labels, intakes, pages } }`.
/// `null` when the response has no `counts` object.
pub fn project_summary_section(summary: &Value) -> Value {
    match summary.get("counts") {
        Some(c) if c.is_object() => json!({ "counts": c }),
        _ => Value::Null,
    }
}

/// Hint for an archived project (`archived_at` set), or `None`. Checked on CE 1.4.2: the
/// project stays in `projects list` and `projects get`, but work items, states, cycles and
/// modules read as empty lists (single gets are 404) until it is unarchived.
pub fn archived_project_hint(project: &Value) -> Option<String> {
    let at = project.get("archived_at").and_then(|x| x.as_str())?;
    Some(format!(
        "This project is archived (since {at}): work-item, state, cycle and module lists read as empty and single gets return 404. Restore it with `plane-cli --json projects unarchive --id <PROJECT>`."
    ))
}

/// Full `projects context` payload (`data` body). `now_utc` (`YYYY-MM-DDTHH:MM:SS`) dates
/// the cycle statuses.
#[allow(clippy::too_many_arguments)]
pub fn build_project_context(
    project_ref: &str,
    project: &Value,
    catalog_entry: Option<&ProjectEntry>,
    states: &[StateEntry],
    labels: &[LabelEntry],
    modules_list: Option<&Value>,
    cycles_list: Option<&Value>,
    now_utc: &str,
) -> Value {
    let id = project
        .get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .or_else(|| catalog_entry.map(|p| p.id.to_string()));
    let identifier = project
        .get("identifier")
        .and_then(|x| x.as_str())
        .map(String::from)
        .or_else(|| catalog_entry.map(|p| p.identifier.clone()));
    let name = project
        .get("name")
        .and_then(|x| x.as_str())
        .map(String::from)
        .or_else(|| catalog_entry.map(|p| p.name.clone()));

    let capabilities = project_capabilities(project);
    let states_sorted: Vec<Value> = states_sorted(states)
        .into_iter()
        .map(state_record)
        .collect();
    let labels_sorted: Vec<Value> = labels_sorted(labels)
        .into_iter()
        .map(label_record)
        .collect();

    let modules = modules_list.map(module_summaries);
    let cycles = cycles_list.map(|v| cycle_summaries(v, now_utc));

    json!({
        "kind": "project_context",
        "project": project_ref,
        "identity": {
            "id": id,
            "identifier": identifier,
            "name": name,
        },
        "capabilities": capabilities,
        "states": states_by_group(states),
        "states_flat": states_sorted,
        "labels": labels_sorted,
        "modules": modules,
        "cycles": cycles,
        "hints": agent_hints(&capabilities, states, labels),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::states_from_value;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn states_from_api_includes_metadata() {
        let v = json!({
            "results": [{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "name": "Todo",
                "group": "unstarted",
                "color": "#fff",
                "sequence": 1.0,
                "default": true
            }]
        });
        let s = states_from_value(&v).unwrap();
        assert_eq!(s[0].color.as_deref(), Some("#fff"));
        assert_eq!(s[0].sequence, Some(1.0));
        assert_eq!(s[0].is_default, Some(true));
    }

    #[test]
    fn states_by_group_buckets() {
        let states = vec![
            StateEntry {
                id: Uuid::nil(),
                name: "Backlog".into(),
                group: Some("backlog".into()),
                color: None,
                sequence: Some(0.0),
                is_default: None,
            },
            StateEntry {
                id: Uuid::max(),
                name: "Todo".into(),
                group: Some("unstarted".into()),
                color: None,
                sequence: Some(1.0),
                is_default: Some(true),
            },
        ];
        let grouped = states_by_group(&states);
        assert_eq!(grouped["backlog"].as_array().unwrap().len(), 1);
        assert_eq!(grouped["unstarted"].as_array().unwrap().len(), 1);
        assert_eq!(grouped["unstarted"][0]["name"].as_str(), Some("Todo"));
    }

    #[test]
    fn build_context_shape() {
        let project = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "identifier": "ENG",
            "name": "Engineering",
            "module_view": true,
            "cycle_view": false
        });
        let ctx = build_project_context("ENG", &project, None, &[], &[], None, None, "");
        assert_eq!(ctx["kind"], "project_context");
        assert!(ctx["hints"].as_array().unwrap().len() >= 3);
        assert!(ctx["cycles"].is_null());
        assert!(ctx["hints"][0]
            .as_str()
            .unwrap()
            .contains("no workflow states"));
    }

    #[test]
    fn build_context_lists_cycles() {
        let project = json!({"id": "550e8400-e29b-41d4-a716-446655440000", "cycle_view": true});
        let cycles = json!({"results": [{
            "id": "04e9c942-56b1-44b9-8362-9aa9a99707ea",
            "name": "Sprint 12",
            "start_date": "2030-11-20T06:03:30.936255Z",
            "end_date": "2030-12-24T13:59:00Z",
            "archived_at": null
        }]});
        let ctx = build_project_context(
            "ENG",
            &project,
            None,
            &[],
            &[],
            None,
            Some(&cycles),
            "2030-11-21T03:00:00",
        );
        assert_eq!(ctx["cycles"][0]["name"], "Sprint 12");
        assert_eq!(ctx["cycles"][0]["status"], "current");
        assert!(ctx["hints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h.as_str().unwrap().contains("cycles list")));
    }

    #[test]
    fn summary_section_keeps_counts_only() {
        let v = json!({"counts": {"issues": 8, "modules": 6}, "id": "x", "name": "Engineering"});
        assert_eq!(
            project_summary_section(&v),
            json!({"counts": {"issues": 8, "modules": 6}})
        );
        assert!(project_summary_section(&json!({})).is_null());
    }

    #[test]
    fn archived_hint_only_for_archived_projects() {
        assert!(archived_project_hint(&json!({"archived_at": null})).is_none());
        let h = archived_project_hint(&json!({"archived_at": "2030-11-21T09:36:19Z"})).unwrap();
        assert!(h.contains("projects unarchive"));
    }

    #[test]
    fn archived_project_gets_no_states_create_hint() {
        let live = project_capabilities(&json!({"archived_at": null}));
        assert!(agent_hints(&live, &[], &[])[0].contains("states create"));
        let archived = project_capabilities(&json!({"archived_at": "2030-11-21T09:36:19Z"}));
        assert_eq!(archived["archived"], true);
        assert!(!agent_hints(&archived, &[], &[])
            .iter()
            .any(|h| h.contains("states create")));
    }
}
