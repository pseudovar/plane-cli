//! `plane-cli projects features`: show or switch a project's cycles, modules, pages, intake
//! and views (`cycle_view`, `module_view`, `page_view`, `intake_view`, `issue_views_view`).
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use crate::output;
use clap::ValueHint;
use plane_core::{
    parse_on_off, project_features, resolve_project_id, CoreError, PlaneApi, ProjectEntry,
    PROJECT_FEATURES,
};
use serde_json::{json, Value};

#[derive(clap::Args, Clone)]
pub struct FeaturesArgs {
    #[arg(
        long,
        value_name = "PROJECT",
        value_hint = ValueHint::Other,
        help = "Project identifier or UUID"
    )]
    pub id: String,
    #[arg(long, value_name = "on|off", help = "Cycles (`cycle_view`)")]
    pub cycles: Option<String>,
    #[arg(long, value_name = "on|off", help = "Modules (`module_view`)")]
    pub modules: Option<String>,
    #[arg(long, value_name = "on|off", help = "Pages (`page_view`)")]
    pub pages: Option<String>,
    #[arg(long, value_name = "on|off", help = "Intake (`intake_view`)")]
    pub intake: Option<String>,
    #[arg(long, value_name = "on|off", help = "Saved views (`issue_views_view`)")]
    pub views: Option<String>,
}

impl FeaturesArgs {
    fn requested(&self) -> [(&'static str, Option<&String>); 5] {
        [
            ("cycles", self.cycles.as_ref()),
            ("modules", self.modules.as_ref()),
            ("pages", self.pages.as_ref()),
            ("intake", self.intake.as_ref()),
            ("views", self.views.as_ref()),
        ]
    }
}

fn field_for(name: &str) -> &'static str {
    PROJECT_FEATURES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .expect("known feature")
}

pub(super) fn run(
    args: &FeaturesArgs,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
) -> Result<(), CoreError> {
    let pid = resolve_project_id(&args.id, catalog)?;
    let mut patch = serde_json::Map::new();
    let mut changed = Vec::new();
    for (name, raw) in args.requested() {
        if let Some(raw) = raw {
            let on = parse_on_off(&format!("--{name}"), raw)?;
            patch.insert(field_for(name).into(), json!(on));
            changed.push(name);
        }
    }
    let project = if patch.is_empty() {
        api.get_project(&pid)?
    } else {
        api.update_project(&pid, &Value::Object(patch))?
    };
    let data = json!({
        "id": pid.to_string(),
        "identifier": project.get("identifier").cloned().unwrap_or(Value::Null),
        "features": project_features(&project),
        "updated": changed,
    });
    output::emit_success(json_mode, data, None, false)?;
    Ok(())
}
