use crate::complete_catalog::{
    label_records_json, sorted_label_names, sorted_module_uuid_strings, sorted_project_identifiers,
    sorted_state_names, state_records_json,
};
use crate::dispatch::{Cli, CompleteSubCmd};
use crate::output::write_stderr_line;
use plane_core::{cycle_summaries, now_utc_timestamp, states_by_group};
use plane_core::{
    print_json, resolve_project_id, CoreError, PageParams, PlaneApi, ResponseEnvelope,
};
use serde_json::json;

pub fn run(
    cli: &Cli,
    json_mode: bool,
    cmd: &CompleteSubCmd,
    api: &mut PlaneApi,
) -> Result<(), CoreError> {
    let refresh = cli.refresh;
    match cmd {
        CompleteSubCmd::Projects => {
            let catalog = api.fetch_projects_catalog(refresh)?;
            let ids = sorted_project_identifiers(&catalog);
            if json_mode {
                let env = ResponseEnvelope::success(json!({
                    "kind": "projects",
                    "identifiers": ids,
                }));
                print_json(&env)?;
            } else {
                for id in ids {
                    println!("{id}");
                }
            }
        }
        CompleteSubCmd::States { project, extra } => {
            let mut warnings = vec![];
            let project_str =
                resolve_complete_project_arg(project.as_deref(), extra, &mut warnings)?;
            let catalog = api.fetch_projects_catalog(refresh)?;
            let pid = resolve_project_id(&project_str, &catalog)?;
            let states = api.fetch_states_catalog(&pid, refresh)?;
            let names = sorted_state_names(&states);
            if json_mode {
                let env = ResponseEnvelope::success(json!({
                    "kind": "states",
                    "project": project_str,
                    "names": names,
                    "states": state_records_json(&states),
                    "by_group": states_by_group(&states),
                }))
                .with_warnings(warnings);
                print_json(&env)?;
            } else {
                for w in &warnings {
                    let _ = write_stderr_line(&format!("warning: {w}"));
                }
                for n in names {
                    println!("{n}");
                }
            }
        }
        CompleteSubCmd::Labels { project, extra } => {
            let mut warnings = vec![];
            let project_str =
                resolve_complete_project_arg(project.as_deref(), extra, &mut warnings)?;
            let catalog = api.fetch_projects_catalog(refresh)?;
            let pid = resolve_project_id(&project_str, &catalog)?;
            let labels = api.fetch_labels_catalog(&pid, refresh)?;
            let names = sorted_label_names(&labels);
            if json_mode {
                let env = ResponseEnvelope::success(json!({
                    "kind": "labels",
                    "project": project_str,
                    "names": names,
                    "labels": label_records_json(&labels),
                }))
                .with_warnings(warnings);
                print_json(&env)?;
            } else {
                for w in &warnings {
                    let _ = write_stderr_line(&format!("warning: {w}"));
                }
                for n in names {
                    println!("{n}");
                }
            }
        }
        CompleteSubCmd::Modules { project, extra } => {
            let mut warnings = vec![];
            let project_str =
                resolve_complete_project_arg(project.as_deref(), extra, &mut warnings)?;
            let catalog = api.fetch_projects_catalog(refresh)?;
            let pid = resolve_project_id(&project_str, &catalog)?;
            let page = PageParams::all();
            let (v, _, _) = api.list_modules(&pid, &page)?;
            let id_strs = sorted_module_uuid_strings(&v)?;
            if json_mode {
                let env = ResponseEnvelope::success(json!({
                    "kind": "modules",
                    "project": project_str,
                    "uuids": id_strs,
                }))
                .with_warnings(warnings);
                print_json(&env)?;
            } else {
                for w in &warnings {
                    let _ = write_stderr_line(&format!("warning: {w}"));
                }
                for s in id_strs {
                    println!("{s}");
                }
            }
        }
        CompleteSubCmd::Cycles { project, extra } => {
            let mut warnings = vec![];
            let project_str =
                resolve_complete_project_arg(project.as_deref(), extra, &mut warnings)?;
            let catalog = api.fetch_projects_catalog(refresh)?;
            let pid = resolve_project_id(&project_str, &catalog)?;
            let page = PageParams::all();
            let (v, _, _) = api.list_cycles(&pid, None, &page)?;
            let cycles = cycle_summaries(&v, &now_utc_timestamp());
            let names: Vec<String> = cycles
                .iter()
                .filter_map(|c| c.get("name").and_then(|x| x.as_str()).map(String::from))
                .collect();
            if json_mode {
                let env = ResponseEnvelope::success(json!({
                    "kind": "cycles",
                    "project": project_str,
                    "names": names,
                    "cycles": cycles,
                }))
                .with_warnings(warnings);
                print_json(&env)?;
            } else {
                for w in &warnings {
                    let _ = write_stderr_line(&format!("warning: {w}"));
                }
                for n in names {
                    println!("{n}");
                }
            }
        }
    }
    Ok(())
}

/// Resolve project identifier from --project flag or positional extras. Emits warning when positional used.
fn resolve_complete_project_arg(
    flag: Option<&str>,
    extra: &[String],
    warnings: &mut Vec<String>,
) -> Result<String, CoreError> {
    if let Some(p) = flag {
        return Ok(p.to_string());
    }
    // Filter out the literal word "project" in case agent wrote `complete states project <UUID>`
    let positionals: Vec<&str> = extra
        .iter()
        .map(String::as_str)
        .filter(|s| *s != "project")
        .collect();
    match positionals.as_slice() {
        [p] => {
            warnings.push(format!(
                "project {p:?} passed as positional arg; use --project {p} to suppress this warning"
            ));
            Ok(p.to_string())
        }
        [] => Err(CoreError::Usage(
            "--project <PROJECT> is required (project identifier, e.g. ENG, or UUID)".into(),
        )),
        _ => Err(CoreError::Usage(format!(
            "ambiguous positional args {:?}; use --project <PROJECT>",
            extra
        ))),
    }
}
