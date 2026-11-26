//! `plane-cli labels …`: project label CRUD. Writes drop the cached labels catalog.
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::PageArgs;
use crate::body::merge_body;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    normalize_color, resolve_label_ref, resolve_project_id, CoreError, PlaneApi, ProjectEntry,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(clap::Args, Clone, Default)]
pub struct LabelFields {
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    #[arg(long, value_name = "#RRGGBB", help = "Hex colour, e.g. #F59E0B")]
    pub color: Option<String>,
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    #[arg(
        long,
        value_name = "LABEL",
        value_hint = ValueHint::Other,
        help = "Parent label UUID or exact name (nests this label under it)"
    )]
    pub parent: Option<String>,
    #[arg(long, value_name = "N", help = "Sort position (`sort_order`)")]
    pub sort_order: Option<f64>,
    #[arg(long = "body", value_name = "PATH|-", value_hint = ValueHint::AnyPath)]
    pub json: Option<String>,
}

#[derive(Subcommand)]
pub enum LabelsCmd {
    /// List labels
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
    /// Create a label (`--name` required)
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[command(flatten)]
        fields: LabelFields,
    },
    /// Rename, recolour, re-parent or re-order a label
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
            value_name = "LABEL",
            value_hint = ValueHint::Other,
            help = "Label UUID or exact name"
        )]
        label: String,
        #[command(flatten)]
        fields: LabelFields,
    },
    /// Delete a label (no confirmation; Plane removes it from work items)
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
            value_name = "LABEL",
            value_hint = ValueHint::Other,
            help = "Label UUID or exact name"
        )]
        label: String,
    },
}

pub fn labels_cmd_name(cmd: &LabelsCmd) -> &'static str {
    match cmd {
        LabelsCmd::List { .. } => "list",
        LabelsCmd::Create { .. } => "create",
        LabelsCmd::Update { .. } => "update",
        LabelsCmd::Delete { .. } => "delete",
    }
}

fn label_body(api: &mut PlaneApi, pid: &Uuid, f: &LabelFields) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(n) = &f.name {
        p.insert("name".into(), json!(n));
    }
    if let Some(c) = &f.color {
        p.insert("color".into(), json!(normalize_color("--color", c)?));
    }
    if let Some(d) = &f.description {
        p.insert("description".into(), json!(d));
    }
    if let Some(parent) = &f.parent {
        let labels = api.fetch_labels_catalog(pid, true)?;
        let id = resolve_label_ref(&labels, parent)?;
        p.insert("parent".into(), json!(id.to_string()));
    }
    if let Some(s) = f.sort_order {
        p.insert("sort_order".into(), json!(s));
    }
    merge_body(f.json.as_deref(), Value::Object(p))
}

pub(super) fn run(
    cmd: &LabelsCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
) -> Result<(), CoreError> {
    match cmd {
        LabelsCmd::List { project, page } => {
            let pid = resolve_project_id(project, catalog)?;
            let (v, nc, tr) = api.list_labels(&pid, &page.params())?;
            output::emit_success(json_mode, v, nc, tr)?;
        }
        LabelsCmd::Create { project, fields } => {
            let pid = resolve_project_id(project, catalog)?;
            let body = label_body(api, &pid, fields)?;
            if body.get("name").and_then(|x| x.as_str()).is_none() {
                return Err(CoreError::Usage(
                    "labels create needs --name (or `name` in --body)".into(),
                ));
            }
            let v = api.create_label(&pid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        LabelsCmd::Update {
            project,
            label,
            fields,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let labels = api.fetch_labels_catalog(&pid, true)?;
            let lid = resolve_label_ref(&labels, label)?;
            let body = label_body(api, &pid, fields)?;
            if body.as_object().map(|m| m.is_empty()).unwrap_or(true) {
                return Err(CoreError::Usage(
                    "nothing to update; pass --name, --color, --description, --parent, --sort-order or --body"
                        .into(),
                ));
            }
            let v = api.update_label(&pid, &lid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        LabelsCmd::Delete { project, label } => {
            let pid = resolve_project_id(project, catalog)?;
            let labels = api.fetch_labels_catalog(&pid, true)?;
            let lid = resolve_label_ref(&labels, label)?;
            api.delete_label(&pid, &lid)?;
            output::emit_empty_success(json_mode)?;
        }
    }
    Ok(())
}
