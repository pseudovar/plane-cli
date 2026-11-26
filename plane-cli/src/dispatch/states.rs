//! `plane-cli states …`: workflow state CRUD. Writes drop the cached states catalog.
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::PageArgs;
use crate::body::merge_body;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    default_state_color, normalize_color, push_warning, resolve_project_id, resolve_state_ref,
    CoreError, PlaneApi, ProjectEntry, STATE_GROUPS,
};
use serde_json::{json, Value};

#[derive(clap::Args, Clone, Default)]
pub struct StateFields {
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    #[arg(
        long,
        value_name = "GROUP",
        value_parser = clap::builder::PossibleValuesParser::new(STATE_GROUPS),
        help = "State group (board column type)"
    )]
    pub group: Option<String>,
    #[arg(
        long,
        value_name = "#RRGGBB",
        help = "Hex colour; create defaults to Plane's colour for the group"
    )]
    pub color: Option<String>,
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    #[arg(
        long,
        value_name = "N",
        help = "Position within the workflow (Plane's defaults step by 10000)"
    )]
    pub sequence: Option<f64>,
    #[arg(
        long,
        help = "Make this the project's default state for new work items"
    )]
    pub default: bool,
    #[arg(long = "body", value_name = "PATH|-", value_hint = ValueHint::AnyPath)]
    pub json: Option<String>,
}

#[derive(Subcommand)]
pub enum StatesCmd {
    /// List workflow states
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
    /// Create a state (`--name` and `--group` required)
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[command(flatten)]
        fields: StateFields,
    },
    /// Rename, regroup, recolour, reorder a state or make it the default
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
            value_name = "STATE",
            value_hint = ValueHint::Other,
            help = "State UUID or exact name"
        )]
        state: String,
        #[command(flatten)]
        fields: StateFields,
    },
    /// Delete a state (Plane refuses the default state and states that still hold work items)
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
            value_name = "STATE",
            value_hint = ValueHint::Other,
            help = "State UUID or exact name"
        )]
        state: String,
    },
}

pub fn states_cmd_name(cmd: &StatesCmd) -> &'static str {
    match cmd {
        StatesCmd::List { .. } => "list",
        StatesCmd::Create { .. } => "create",
        StatesCmd::Update { .. } => "update",
        StatesCmd::Delete { .. } => "delete",
    }
}

fn state_body(f: &StateFields) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(n) = &f.name {
        p.insert("name".into(), json!(n));
    }
    if let Some(g) = &f.group {
        p.insert("group".into(), json!(g));
    }
    if let Some(c) = &f.color {
        p.insert("color".into(), json!(normalize_color("--color", c)?));
    }
    if let Some(d) = &f.description {
        p.insert("description".into(), json!(d));
    }
    if let Some(s) = f.sequence {
        p.insert("sequence".into(), json!(s));
    }
    if f.default {
        p.insert("default".into(), json!(true));
    }
    merge_body(f.json.as_deref(), Value::Object(p))
}

/// CE 1.4.2 lets several states be `default` at once (an update that sets one does not clear the
/// others), so after making `new_default` the default, clear the flag everywhere else.
/// The main write already succeeded, so failures here are warnings, not errors.
fn clear_other_defaults(api: &mut PlaneApi, pid: &uuid::Uuid, new_default: &Value) {
    let keep = new_default.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let states = match api.fetch_states_catalog(pid, true) {
        Ok(s) => s,
        Err(e) => {
            push_warning(format!(
                "could not check other states for a second `default` flag ({e}); run `states list` and clear it with `states update --body`"
            ));
            return;
        }
    };
    for s in states
        .iter()
        .filter(|s| s.is_default == Some(true) && s.id.to_string() != keep)
    {
        match api.update_state(pid, &s.id, &json!({ "default": false })) {
            Ok(_) => {
                push_warning(format!(
                    "cleared `default` on {:?} so the project has one default state",
                    s.name
                ));
            }
            Err(e) => push_warning(format!(
                "{:?} is still marked default too; clearing it failed ({e})",
                s.name
            )),
        }
    }
}

pub(super) fn run(
    cmd: &StatesCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
) -> Result<(), CoreError> {
    match cmd {
        StatesCmd::List { project, page } => {
            let pid = resolve_project_id(project, catalog)?;
            let (v, nc, tr) = api.list_states(&pid, &page.params())?;
            output::emit_success(json_mode, v, nc, tr)?;
        }
        StatesCmd::Create { project, fields } => {
            let pid = resolve_project_id(project, catalog)?;
            let mut body = state_body(fields)?;
            if body.get("name").and_then(|x| x.as_str()).is_none() {
                return Err(CoreError::Usage(
                    "states create needs --name (or `name` in --body)".into(),
                ));
            }
            let Some(group) = body.get("group").and_then(|x| x.as_str()).map(String::from) else {
                return Err(CoreError::Usage(
                    "states create needs --group (backlog, unstarted, started, completed, cancelled)"
                        .into(),
                ));
            };
            if body.get("color").is_none() {
                let c = default_state_color(&group);
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("color".into(), json!(c));
                }
                push_warning(format!(
                    "no --color given; used Plane's default for {group} ({c})"
                ));
            }
            let mut v = api.create_state(&pid, &body)?;
            // CE 1.4.2 ignores `sequence` on create (it appends after the last state) but
            // honours it on update, so apply a requested sequence with a follow-up PATCH.
            // The state exists by now, so a failure here is a warning, not an error.
            let wanted = body.get("sequence").and_then(|x| x.as_f64());
            let got = v.get("sequence").and_then(|x| x.as_f64());
            if let (Some(want), Some(have)) = (wanted, got) {
                if (want - have).abs() > f64::EPSILON {
                    let sid = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());
                    match sid.map(|sid| api.update_state(&pid, &sid, &json!({ "sequence": want }))) {
                        Some(Ok(updated)) => {
                            v = updated;
                            push_warning(format!(
                                "Plane placed the new state at sequence {have}; plane-cli moved it to {want} with an update"
                            ));
                        }
                        Some(Err(e)) => push_warning(format!(
                            "state created at sequence {have}; moving it to {want} failed ({e}); retry with `states update --sequence {want}`"
                        )),
                        None => push_warning(format!(
                            "state created at sequence {have}, but the response had no id to move it to {want}"
                        )),
                    }
                }
            }
            if body.get("default").and_then(|x| x.as_bool()) == Some(true) {
                clear_other_defaults(api, &pid, &v);
            }
            output::emit_success(json_mode, v, None, false)?;
        }
        StatesCmd::Update {
            project,
            state,
            fields,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let states = api.fetch_states_catalog(&pid, true)?;
            let sid = resolve_state_ref(&states, state)?;
            let body = state_body(fields)?;
            if body.as_object().map(|m| m.is_empty()).unwrap_or(true) {
                return Err(CoreError::Usage(
                    "nothing to update; pass --name, --group, --color, --description, --sequence, --default or --body"
                        .into(),
                ));
            }
            let v = api.update_state(&pid, &sid, &body)?;
            if body.get("default").and_then(|x| x.as_bool()) == Some(true) {
                clear_other_defaults(api, &pid, &v);
            }
            output::emit_success(json_mode, v, None, false)?;
        }
        StatesCmd::Delete { project, state } => {
            let pid = resolve_project_id(project, catalog)?;
            let states = api.fetch_states_catalog(&pid, true)?;
            let sid = resolve_state_ref(&states, state)?;
            api.delete_state(&pid, &sid)?;
            output::emit_empty_success(json_mode)?;
        }
    }
    Ok(())
}
