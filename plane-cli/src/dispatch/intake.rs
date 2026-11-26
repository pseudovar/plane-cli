//! `plane-cli intake …`: the project intake queue (needs `projects features --intake on`).
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::helpers::resolve_work_item_pair;
use super::PageArgs;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    annotate_intake_statuses, load_json_source, merge_objects, parse_composite_work_item,
    parse_intake_status, resolve_project_id, CoreError, PageParams, PlaneApi, ProjectEntry,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(clap::Args, Clone, Default)]
pub struct IntakeIssueFields {
    #[arg(long, value_name = "TEXT", help = "Work item title")]
    pub name: Option<String>,
    #[arg(
        long,
        value_name = "HTML",
        help = "Work item description (sent as `description_html`)"
    )]
    pub description: Option<String>,
    #[arg(
        long,
        value_name = "PRIORITY",
        value_parser = clap::builder::PossibleValuesParser::new(["urgent", "high", "medium", "low", "none"])
    )]
    pub priority: Option<String>,
}

#[derive(Subcommand)]
pub enum IntakeCmd {
    /// List intake work items (adds `status_name`)
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Show one intake work item
    Get {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite id (PROJ-123) or work item UUID of the intake item"
        )]
        work_item: String,
    },
    /// Submit a work item to intake (`--name` required); it starts as pending
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[command(flatten)]
        fields: IntakeIssueFields,
        #[arg(
            long = "body",
            value_name = "PATH|-",
            value_hint = ValueHint::AnyPath,
            help = "JSON: `{\"issue\": {…}}` or the work item fields alone"
        )]
        json: Option<String>,
    },
    /// Triage (accept, reject, snooze, mark duplicate) or edit an intake work item
    Update {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite id (PROJ-123) or work item UUID of the intake item"
        )]
        work_item: String,
        #[arg(
            long,
            value_name = "STATUS",
            value_parser = clap::builder::PossibleValuesParser::new(["pending", "rejected", "snoozed", "accepted", "duplicate"])
        )]
        status: Option<String>,
        #[arg(
            long,
            value_name = "DATETIME",
            help = "With --status snoozed: when it comes back (ISO 8601, e.g. 2030-12-01T00:00:00Z)"
        )]
        snoozed_till: Option<String>,
        #[arg(
            long,
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "With --status duplicate: the work item it duplicates (PROJ-123 or UUID)"
        )]
        duplicate_to: Option<String>,
        #[command(flatten)]
        fields: IntakeIssueFields,
        #[arg(long = "body", value_name = "PATH|-", value_hint = ValueHint::AnyPath)]
        json: Option<String>,
        #[arg(
            long,
            value_name = "TEXT",
            help = "Why: posted as a comment on the work item after the update; the project policy can require it for priority changes"
        )]
        reason: Option<String>,
    },
    /// Delete an intake work item (also deletes the work item unless it was accepted)
    Delete {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite id (PROJ-123) or work item UUID of the intake item"
        )]
        work_item: String,
    },
}

pub fn intake_cmd_name(cmd: &IntakeCmd) -> &'static str {
    match cmd {
        IntakeCmd::List { .. } => "list",
        IntakeCmd::Get { .. } => "get",
        IntakeCmd::Create { .. } => "create",
        IntakeCmd::Update { .. } => "update",
        IntakeCmd::Delete { .. } => "delete",
    }
}

fn issue_patch(f: &IntakeIssueFields) -> serde_json::Map<String, Value> {
    let mut p = serde_json::Map::new();
    if let Some(n) = &f.name {
        p.insert("name".into(), json!(n));
    }
    if let Some(d) = &f.description {
        p.insert("description_html".into(), json!(d));
    }
    if let Some(pr) = &f.priority {
        p.insert("priority".into(), json!(pr));
    }
    p
}

/// `--body` may be the full request (`{"issue": {...}, "status": …}`) or bare work item
/// fields; the flags are merged into `issue`.
fn intake_body(
    json_path: Option<&str>,
    issue_flags: serde_json::Map<String, Value>,
    top_level: serde_json::Map<String, Value>,
) -> Result<Value, CoreError> {
    let base = load_json_source(json_path)?.unwrap_or(json!({}));
    let mut base = match base {
        Value::Object(m) => m,
        _ => return Err(CoreError::Usage("--body must be a JSON object".into())),
    };
    let issue_base = if base.contains_key("issue") {
        base.remove("issue").unwrap_or(json!({}))
    } else {
        let keep = [
            "status",
            "snoozed_till",
            "duplicate_to",
            "source",
            "source_email",
        ];
        let (top, issue): (Vec<_>, Vec<_>) = std::mem::take(&mut base)
            .into_iter()
            .partition(|(k, _)| keep.contains(&k.as_str()));
        base = top.into_iter().collect();
        Value::Object(issue.into_iter().collect())
    };
    let issue = merge_objects(issue_base, Value::Object(issue_flags));
    if issue.as_object().map(|m| !m.is_empty()).unwrap_or(false) {
        base.insert("issue".into(), issue);
    }
    for (k, v) in top_level {
        base.insert(k, v);
    }
    Ok(Value::Object(base))
}

/// The work item UUID of an intake row. The API addresses intake rows by work item UUID,
/// and `work-items/PROJ-N/` does not find items still pending in intake (CE 1.4.2), so
/// match against the intake list: work item UUID, intake row id, or `PROJ-N`.
fn intake_issue_for(rows: &[Value], project_identifier: Option<&str>, raw: &str) -> Option<Uuid> {
    let raw = raw.trim();
    let issue_of = |r: &Value| {
        r.get("issue")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    if let Ok(u) = Uuid::parse_str(raw) {
        let us = u.to_string();
        return rows
            .iter()
            .find(|r| {
                r.get("issue").and_then(|x| x.as_str()) == Some(us.as_str())
                    || r.get("id").and_then(|x| x.as_str()) == Some(us.as_str())
            })
            .and_then(issue_of);
    }
    let (ident, seq) = parse_composite_work_item(raw)?;
    if let Some(p) = project_identifier {
        if !p.eq_ignore_ascii_case(&ident) {
            return None;
        }
    }
    rows.iter()
        .find(|r| {
            r.get("issue_detail")
                .and_then(|d| d.get("sequence_id"))
                .and_then(|x| x.as_u64())
                == Some(seq)
        })
        .and_then(issue_of)
}

fn intake_item(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    project: &str,
    work_item: &str,
) -> Result<(Uuid, Uuid), CoreError> {
    let pid = resolve_project_id(project, catalog)?;
    let page = PageParams::all();
    let (list, _, _) = api.list_intake(&pid, &page)?;
    let rows = list
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let ident = catalog
        .iter()
        .find(|p| p.id == pid)
        .map(|p| p.identifier.as_str());
    if let Some(iid) = intake_issue_for(&rows, ident, work_item) {
        return Ok((pid, iid));
    }
    if let Ok(u) = Uuid::parse_str(work_item.trim()) {
        // Not in the list (yet); let the API answer for this UUID.
        return Ok((pid, u));
    }
    Err(CoreError::Resolve(format!(
        "no intake item {work_item} in {project}; see `intake list --project {project}`"
    )))
}

pub(super) fn run(
    cmd: &IntakeCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
) -> Result<(), CoreError> {
    match cmd {
        IntakeCmd::List { project, page } => {
            let pid = resolve_project_id(project, catalog)?;
            let (mut v, nc, tr) = api.list_intake(&pid, &page.params())?;
            annotate_intake_statuses(&mut v);
            output::emit_success(json_mode, v, nc, tr)?;
        }
        IntakeCmd::Get { project, work_item } => {
            let (pid, iid) = intake_item(api, catalog, project, work_item)?;
            let mut v = api.get_intake(&pid, &iid)?;
            annotate_intake_statuses(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        IntakeCmd::Create {
            project,
            fields,
            json,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let body = intake_body(json.as_deref(), issue_patch(fields), Default::default())?;
            let has_name = body
                .get("issue")
                .and_then(|i| i.get("name"))
                .and_then(|x| x.as_str())
                .is_some();
            if !has_name {
                return Err(CoreError::Usage(
                    "intake create needs --name (or `issue.name` in --body)".into(),
                ));
            }
            let issue = body.get("issue").cloned().unwrap_or(Value::Null);
            let warnings =
                super::policy_gate::gate_intake_create(api, catalog, refresh, &pid, &issue)?;
            let mut v = api.create_intake(&pid, &body)?;
            annotate_intake_statuses(&mut v);
            output::emit_success_with_warnings(json_mode, v, None, false, warnings)?;
        }
        IntakeCmd::Update {
            project,
            work_item,
            status,
            snoozed_till,
            duplicate_to,
            fields,
            json,
            reason,
        } => {
            let (pid, iid) = intake_item(api, catalog, project, work_item)?;
            let mut top = serde_json::Map::new();
            if let Some(s) = status {
                top.insert("status".into(), json!(parse_intake_status(s)?));
            }
            if let Some(t) = snoozed_till {
                top.insert("snoozed_till".into(), json!(t));
            }
            if let Some(d) = duplicate_to {
                let (_, dup) =
                    resolve_work_item_pair(api, catalog, refresh, Some(project), d, true)?;
                top.insert("duplicate_to".into(), json!(dup.to_string()));
            }
            let body = intake_body(json.as_deref(), issue_patch(fields), top)?;
            if body.as_object().map(|m| m.is_empty()).unwrap_or(true) {
                return Err(CoreError::Usage(
                    "nothing to update; pass --status, --snoozed-till, --duplicate-to, --name, --description, --priority or --body"
                        .into(),
                ));
            }
            let issue = body.get("issue").cloned().unwrap_or_else(|| json!({}));
            let comment = super::policy_gate::gate_intake_update(
                api,
                catalog,
                refresh,
                &pid,
                &iid,
                &issue,
                reason.as_deref(),
            )?;
            let mut v = api.update_intake(&pid, &iid, &body)?;
            annotate_intake_statuses(&mut v);
            let mut warnings = Vec::new();
            if let Some(html) = comment {
                super::policy_gate::post_reason_comment(
                    api,
                    &pid,
                    &iid,
                    &html,
                    &mut v,
                    &mut warnings,
                );
            }
            output::emit_success_with_warnings(json_mode, v, None, false, warnings)?;
        }
        IntakeCmd::Delete { project, work_item } => {
            let (pid, iid) = intake_item(api, catalog, project, work_item)?;
            super::policy_gate::gate_delete(api, catalog, &pid, "intake item", "intake delete")?;
            api.delete_intake(&pid, &iid)?;
            output::emit_empty_success(json_mode)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{intake_body, intake_issue_for};
    use serde_json::json;

    #[test]
    fn intake_rows_match_issue_row_id_or_composite() {
        let issue = "69448f88-11a9-4196-8169-6424f3074e27";
        let row_id = "f5ed0593-17bf-4b43-b282-d95911614865";
        let rows = vec![json!({"id": row_id, "issue": issue, "issue_detail": {"sequence_id": 4}})];
        for input in [issue, row_id, "WEB-4", "web-4"] {
            assert_eq!(
                intake_issue_for(&rows, Some("WEB"), input)
                    .unwrap()
                    .to_string(),
                issue,
                "{input}"
            );
        }
        assert!(intake_issue_for(&rows, Some("WEB"), "WEB-5").is_none());
        assert!(intake_issue_for(&rows, Some("WEB"), "ENG-4").is_none());
    }

    #[test]
    fn flat_body_fields_move_under_issue() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.json");
        std::fs::write(&p, r#"{"name": "Bug", "status": -2}"#).unwrap();
        let mut flags = serde_json::Map::new();
        flags.insert("priority".into(), json!("high"));
        let v = intake_body(Some(p.to_str().unwrap()), flags, Default::default()).unwrap();
        assert_eq!(
            v,
            json!({"status": -2, "issue": {"name": "Bug", "priority": "high"}})
        );
    }

    #[test]
    fn status_only_update_has_no_issue_key() {
        let mut top = serde_json::Map::new();
        top.insert("status".into(), json!(1));
        let v = intake_body(None, Default::default(), top).unwrap();
        assert_eq!(v, json!({"status": 1}));
    }
}
