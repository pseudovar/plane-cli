//! `plane-cli projects context` — agent-oriented project snapshot.

use crate::dispatch::Cli;
use plane_core::{
    archived_project_hint, build_project_context, now_utc_timestamp, project_summary_section,
    push_warning, resolve_project_id, CoreError, PageParams, PlaneApi,
};
use serde_json::Value;

pub fn run(
    cli: &Cli,
    json_mode: bool,
    project_ref: &str,
    api: &mut PlaneApi,
) -> Result<(), CoreError> {
    if !json_mode {
        return Err(CoreError::Usage(
            "projects context requires --json (machine-readable snapshot)".into(),
        ));
    }
    let refresh = cli.refresh;
    let catalog = api.fetch_projects_catalog(refresh)?;
    let pid = resolve_project_id(project_ref, &catalog)?;
    let project: Value = api.get_project(&pid)?;
    let states = api.fetch_states_catalog(&pid, refresh)?;
    let labels = api.fetch_labels_catalog(&pid, refresh)?;

    let catalog_entry = catalog.iter().find(|p| p.id == pid);
    let modules_list = if project
        .get("module_view")
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
    {
        let page = PageParams::all();
        let (v, _, _) = api.list_modules(&pid, &page)?;
        Some(v)
    } else {
        None
    };

    let cycles_list = if project
        .get("cycle_view")
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
    {
        let page = PageParams::all();
        let (v, _, _) = api.list_cycles(&pid, None, &page)?;
        Some(v)
    } else {
        None
    };

    let data = build_project_context(
        project_ref,
        &project,
        catalog_entry,
        &states,
        &labels,
        modules_list.as_ref(),
        cycles_list.as_ref(),
        &now_utc_timestamp(),
    );
    let mut data = data;
    let identifier = catalog_entry
        .map(|e| e.identifier.clone())
        .or_else(|| {
            project
                .get("identifier")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .unwrap_or_default();
    let summary = api.project_summary(&pid);
    add_extras(&mut data, &project, summary, &identifier);
    crate::output::emit_success(true, data, None, false)?;
    Ok(())
}

/// Adds `summary`, `policy` and the policy / archived-project hints to a context payload.
/// Summary counts are extra context: a failed summary call is a warning and `summary: null`,
/// not a failed snapshot.
fn add_extras(
    data: &mut Value,
    project: &Value,
    summary: Result<Value, CoreError>,
    identifier: &str,
) {
    let summary = match summary {
        Ok(v) => project_summary_section(&v),
        Err(e) => {
            push_warning(format!(
                "summary: `projects summary` failed ({}); `summary` is null",
                e.error_code()
            ));
            Value::Null
        }
    };
    let (policy, hint) = policy_section(identifier);
    if let Some(obj) = data.as_object_mut() {
        obj.insert("summary".into(), summary);
        obj.insert("policy".into(), policy);
        if let Some(Value::Array(hints)) = obj.get_mut("hints") {
            if let Some(h) = hint {
                hints.insert(0, Value::String(h));
            }
            if let Some(h) = archived_project_hint(project) {
                hints.insert(0, Value::String(h));
            }
        }
    }
}

/// The active write policy for `projects context`: `null` without one, `{ "error" }`
/// when the file cannot be loaded (writes to the project are refused until it is fixed).
fn policy_section(identifier: &str) -> (Value, Option<String>) {
    if identifier.is_empty() {
        return (Value::Null, None);
    }
    match crate::policy::load(identifier) {
        Ok(None) => (Value::Null, None),
        Ok(Some(l)) => (
            l.to_json(),
            Some(format!(
                "This project has a write policy (policy.summary): plane-cli refuses writes that break it with error.code policy_violation. Preview a write with --dry-run; `plane-cli --json config policy show --project {identifier}` prints the rules."
            )),
        ),
        Err(e) => (
            serde_json::json!({"error": {"code": e.error_code(), "message": e.to_string()}}),
            Some(format!(
                "The write policy file for {identifier} is invalid, so writes to this project fail until it is fixed (policy.error)."
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base() -> Value {
        json!({"kind": "project_context", "hints": ["Preflight: …"]})
    }

    #[test]
    fn failed_summary_is_a_warning_and_null() {
        let _g = crate::plane_env_files::lock_env();
        let _p = crate::dispatch::policy_gate::tests::PolicyEnv::new("WEB", None);
        plane_core::take_warnings();
        let mut data = base();
        add_extras(
            &mut data,
            &json!({"archived_at": null}),
            Err(CoreError::UnexpectedResponse("boom".into())),
            "WEB",
        );
        assert!(data["summary"].is_null());
        let w = plane_core::take_warnings();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].starts_with("summary:"), "{w:?}");
        assert_eq!(data["hints"][0], "Preflight: …");
    }

    #[test]
    fn summary_counts_and_archived_hint_are_added() {
        let _g = crate::plane_env_files::lock_env();
        let _p = crate::dispatch::policy_gate::tests::PolicyEnv::new("WEB", None);
        let mut data = base();
        add_extras(
            &mut data,
            &json!({"archived_at": "2030-11-21T09:36:19Z"}),
            Ok(json!({"counts": {"issues": 1}, "id": "x"})),
            "WEB",
        );
        assert_eq!(data["summary"], json!({"counts": {"issues": 1}}));
        assert!(data["policy"].is_null());
        let first = data["hints"][0].as_str().unwrap();
        assert!(first.contains("archived"), "{first}");
        assert_eq!(data["hints"][1], "Preflight: …");
    }
}
