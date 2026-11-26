//! Plane API sequence for opening a project’s work-item list in the TUI (catalogs + first page).

use plane_core::{CoreError, LabelEntry, PageParams, PlaneApi, StateEntry};
use serde_json::Value;
use uuid::Uuid;

pub struct ProjectWorkListOpen {
    pub items: Vec<Value>,
    pub next_cursor: Option<String>,
    pub states: Vec<StateEntry>,
    pub labels: Vec<LabelEntry>,
}

/// Loads workflow states, labels, and the first page of work items for a project.
/// Interactive TUI does not apply CLI `--strict-catalog` body normalization; see `docs/src/tui.md`.
pub fn load_project_work_list_open(
    api: &mut PlaneApi,
    project_id: Uuid,
) -> Result<ProjectWorkListOpen, CoreError> {
    let states = api.fetch_states_catalog(&project_id, false)?;
    let labels = api.fetch_labels_catalog(&project_id, false)?;
    let page = PageParams {
        per_page: Some(50),
        cursor: None,
        fetch_all: false,
    };
    let (v, next_cursor, _) = api.list_work_items(&project_id, &page)?;
    let items = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(ProjectWorkListOpen {
        items,
        next_cursor,
        states,
        labels,
    })
}
