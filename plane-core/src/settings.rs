//! Helpers for project settings commands: states, labels, features, members, intake.
//!
//! Values here were checked against Plane Community Edition 1.4.2 (`/api/schema/` and live
//! calls).

use crate::catalog::{LabelEntry, StateEntry};
use crate::error::CoreError;
use serde_json::{json, Value};
use uuid::Uuid;

/// Workflow state groups (`GroupEnum`). `triage` is reserved for intake.
pub const STATE_GROUPS: &[&str] = &[
    "backlog",
    "unstarted",
    "started",
    "completed",
    "cancelled",
    "triage",
];

/// Colour used by `states create` when `--color` is omitted (Plane's own defaults).
pub fn default_state_color(group: &str) -> &'static str {
    match group {
        "backlog" => "#60646C",
        "unstarted" => "#60646C",
        "started" => "#F59E0B",
        "completed" => "#46A758",
        "cancelled" => "#9AA4BC",
        _ => "#60646C",
    }
}

/// Accept `#RGB` or `#RRGGBB`; a missing `#` is added. Returns the normalized colour.
pub fn normalize_color(flag: &str, raw: &str) -> Result<String, CoreError> {
    let t = raw.trim();
    let hex = t.strip_prefix('#').unwrap_or(t);
    if (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(format!("#{}", hex.to_ascii_uppercase()))
    } else {
        Err(CoreError::Usage(format!(
            "{flag} {raw:?} is not a hex colour; use #RRGGBB, e.g. #F59E0B"
        )))
    }
}

/// Project feature switches: (flag name, Plane project field).
pub const PROJECT_FEATURES: &[(&str, &str)] = &[
    ("cycles", "cycle_view"),
    ("modules", "module_view"),
    ("pages", "page_view"),
    ("intake", "intake_view"),
    ("views", "issue_views_view"),
];

/// `{ "cycles": true, … }` from a project `GET` payload.
pub fn project_features(project: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (name, field) in PROJECT_FEATURES {
        out.insert(
            (*name).to_string(),
            project.get(*field).cloned().unwrap_or(Value::Null),
        );
    }
    Value::Object(out)
}

/// Parse `on|off` (also `true|false`, `yes|no`, `1|0`).
pub fn parse_on_off(flag: &str, raw: &str) -> Result<bool, CoreError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "enable" | "enabled" => Ok(true),
        "off" | "false" | "no" | "0" | "disable" | "disabled" => Ok(false),
        _ => Err(CoreError::Usage(format!("{flag} {raw:?}: use on or off"))),
    }
}

/// Workspace / project role (`RoleEnum`): admin 20, member 15, guest 5.
pub fn parse_role(raw: &str) -> Result<u16, CoreError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "admin" | "20" => Ok(20),
        "member" | "15" => Ok(15),
        "guest" | "5" => Ok(5),
        _ => Err(CoreError::Usage(format!(
            "--role {raw:?}: use admin, member or guest (20, 15, 5)"
        ))),
    }
}

/// Role name for a `RoleEnum` value.
pub fn role_name(role: u64) -> Option<&'static str> {
    match role {
        20 => Some("admin"),
        15 => Some("member"),
        5 => Some("guest"),
        _ => None,
    }
}

/// Intake status (`IntakeWorkItemStatusEnum`).
pub const INTAKE_STATUSES: &[(&str, i64)] = &[
    ("pending", -2),
    ("rejected", -1),
    ("snoozed", 0),
    ("accepted", 1),
    ("duplicate", 2),
];

pub fn parse_intake_status(raw: &str) -> Result<i64, CoreError> {
    let t = raw.trim().to_ascii_lowercase();
    for (name, v) in INTAKE_STATUSES {
        if t == *name || t == v.to_string() {
            return Ok(*v);
        }
    }
    Err(CoreError::Usage(format!(
        "--status {raw:?}: use pending, rejected, snoozed, accepted or duplicate"
    )))
}

pub fn intake_status_name(v: i64) -> Option<&'static str> {
    INTAKE_STATUSES
        .iter()
        .find(|(_, n)| *n == v)
        .map(|(name, _)| *name)
}

/// Adds `status_name` next to the numeric `status` of intake rows (list or single object).
pub fn annotate_intake_statuses(v: &mut Value) {
    for_each_row(v, |row| {
        let name = row
            .get("status")
            .and_then(|x| x.as_i64())
            .and_then(intake_status_name);
        if let (Some(obj), Some(n)) = (row.as_object_mut(), name) {
            obj.insert("status_name".into(), json!(n));
        }
    });
}

/// Adds `role_name` beside a numeric `role` (members and invitations).
pub fn annotate_role_names(v: &mut Value) {
    for_each_row(v, |row| {
        let name = row.get("role").and_then(|x| x.as_u64()).and_then(role_name);
        if let (Some(obj), Some(n)) = (row.as_object_mut(), name) {
            obj.insert("role_name".into(), json!(n));
        }
    });
}

/// Runs `f` on each row of a list response (paginated `results` or a bare array), or on
/// the value itself when it is a single object.
pub fn for_each_row(v: &mut Value, mut f: impl FnMut(&mut Value)) {
    if let Some(arr) = v.get_mut("results").and_then(|x| x.as_array_mut()) {
        arr.iter_mut().for_each(f);
    } else if let Some(arr) = v.as_array_mut() {
        arr.iter_mut().for_each(f);
    } else {
        f(v);
    }
}

/// Exact name match, else a unique case-insensitive one. `Ok(None)` when nothing matches;
/// an error naming the candidate ids when the name is ambiguous.
pub(crate) fn pick_by_name<'a, T>(
    rows: &'a [T],
    name_of: impl Fn(&T) -> &str,
    id_of: impl Fn(&T) -> Uuid,
    raw: &str,
    kind: &str,
) -> Result<Option<Uuid>, CoreError> {
    let wanted = raw.trim();
    let mut hits: Vec<&'a T> = rows.iter().filter(|r| name_of(r) == wanted).collect();
    if hits.is_empty() {
        hits = rows
            .iter()
            .filter(|r| name_of(r).eq_ignore_ascii_case(wanted))
            .collect();
    }
    match hits.len() {
        0 => Ok(None),
        1 => Ok(Some(id_of(hits[0]))),
        n => Err(CoreError::Resolve(format!(
            "ambiguous {kind} name {wanted:?}: {n} matches ({}); pass the UUID",
            hits.iter()
                .map(|r| id_of(r).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Resolve `--state` (UUID or exact name, case-insensitive fallback).
pub fn resolve_state_ref(states: &[StateEntry], raw: &str) -> Result<Uuid, CoreError> {
    if let Ok(u) = Uuid::parse_str(raw.trim()) {
        return Ok(u);
    }
    if let Some(id) = pick_by_name(states, |s| &s.name, |s| s.id, raw, "state")? {
        return Ok(id);
    }
    let names: Vec<&str> = states.iter().map(|s| s.name.as_str()).collect();
    let known = if names.is_empty() {
        "the project has no states".to_string()
    } else {
        format!("known states: {}", names.join(", "))
    };
    Err(CoreError::Resolve(format!(
        "unknown state {raw:?}; {known}. Use the state UUID or exact name (`states list --project <PROJECT>`)"
    )))
}

/// Resolve `--label` / `--parent` (UUID or exact name, case-insensitive fallback).
pub fn resolve_label_ref(labels: &[LabelEntry], raw: &str) -> Result<Uuid, CoreError> {
    if let Ok(u) = Uuid::parse_str(raw.trim()) {
        return Ok(u);
    }
    if let Some(id) = pick_by_name(labels, |l| &l.name, |l| l.id, raw, "label")? {
        return Ok(id);
    }
    let names: Vec<&str> = labels.iter().map(|l| l.name.as_str()).collect();
    let known = if names.is_empty() {
        "the project has no labels".to_string()
    } else {
        format!("known labels: {}", names.join(", "))
    };
    Err(CoreError::Resolve(format!(
        "unknown label {raw:?}; {known}. Use the label UUID or exact name (`labels list --project <PROJECT>`)"
    )))
}

/// Resolve a workspace member from a UUID, email or display name (`members/` rows).
pub fn resolve_member_ref(members: &Value, raw: &str) -> Result<Uuid, CoreError> {
    if let Ok(u) = Uuid::parse_str(raw.trim()) {
        return Ok(u);
    }
    let wanted = raw.trim();
    let rows = members.as_array().cloned().unwrap_or_default();
    let field = |r: &Value, k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let hits: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            field(r, "email").eq_ignore_ascii_case(wanted)
                || field(r, "display_name").eq_ignore_ascii_case(wanted)
        })
        .collect();
    match hits.len() {
        1 => hits[0]
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::Resolve(format!("member {wanted:?} has no id"))),
        0 => Err(CoreError::Resolve(format!(
            "unknown workspace member {wanted:?}; use a user UUID, email or display name from `members list`"
        ))),
        n => Err(CoreError::Resolve(format!(
            "ambiguous member {wanted:?}: {n} matches; pass the user UUID"
        ))),
    }
}

/// MIME type from a file name, for attachment uploads.
pub fn guess_mime(file_name: &str) -> String {
    mime_guess::from_path(file_name)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str, id: Uuid) -> StateEntry {
        StateEntry {
            id,
            name: name.into(),
            group: Some("started".into()),
            color: None,
            sequence: None,
            is_default: None,
        }
    }

    #[test]
    fn colors_are_normalized_or_rejected() {
        assert_eq!(normalize_color("--color", "f59e0b").unwrap(), "#F59E0B");
        assert_eq!(normalize_color("--color", "#abc").unwrap(), "#ABC");
        assert!(normalize_color("--color", "red").is_err());
        assert!(normalize_color("--color", "#12345").is_err());
    }

    #[test]
    fn on_off_and_roles() {
        assert!(parse_on_off("--cycles", "on").unwrap());
        assert!(!parse_on_off("--cycles", "OFF").unwrap());
        assert!(parse_on_off("--cycles", "maybe").is_err());
        assert_eq!(parse_role("Admin").unwrap(), 20);
        assert_eq!(parse_role("15").unwrap(), 15);
        assert_eq!(parse_role("guest").unwrap(), 5);
        assert!(parse_role("owner").is_err());
        assert_eq!(role_name(15), Some("member"));
    }

    #[test]
    fn intake_status_round_trip() {
        assert_eq!(parse_intake_status("accepted").unwrap(), 1);
        assert_eq!(parse_intake_status("-2").unwrap(), -2);
        assert!(parse_intake_status("done").is_err());
        let mut v = json!({"results": [{"status": -2}, {"status": 1}]});
        annotate_intake_statuses(&mut v);
        assert_eq!(v["results"][0]["status_name"], "pending");
        assert_eq!(v["results"][1]["status_name"], "accepted");
    }

    #[test]
    fn features_from_project() {
        let p = json!({"cycle_view": true, "module_view": false, "intake_view": true});
        let f = project_features(&p);
        assert_eq!(f["cycles"], true);
        assert_eq!(f["modules"], false);
        assert_eq!(f["intake"], true);
        assert!(f["pages"].is_null());
    }

    #[test]
    fn state_refs_resolve_by_name() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let states = vec![state("In Progress", a), state("Done", b)];
        assert_eq!(resolve_state_ref(&states, "done").unwrap(), b);
        assert_eq!(resolve_state_ref(&states, &a.to_string()).unwrap(), a);
        let err = resolve_state_ref(&states, "Review")
            .unwrap_err()
            .to_string();
        assert!(err.contains("known states: In Progress, Done"), "{err}");
        let dup = vec![state("Done", a), state("done", b)];
        assert_eq!(resolve_state_ref(&dup, "Done").unwrap(), a);
        let dup2 = vec![state("DONE", a), state("done", b)];
        assert!(resolve_state_ref(&dup2, "Done").is_err());
    }

    #[test]
    fn member_refs_resolve_by_email_or_name() {
        let id = "0fd600ef-6f34-48b6-824e-ca167b1c05c4";
        let members = json!([{"id": id, "email": "bob@example.com", "display_name": "bob"}]);
        assert_eq!(resolve_member_ref(&members, "Bob").unwrap().to_string(), id);
        assert_eq!(
            resolve_member_ref(&members, "bob@example.com")
                .unwrap()
                .to_string(),
            id
        );
        assert!(resolve_member_ref(&members, "nobody").is_err());
    }

    #[test]
    fn mime_from_extension() {
        assert_eq!(guess_mime("notes.txt"), "text/plain");
        assert_eq!(guess_mime("shot.png"), "image/png");
        assert_eq!(guess_mime("blob"), "application/octet-stream");
    }
}
