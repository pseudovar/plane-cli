use crate::self_cmd::UpdateHint;
use crate::skill_cmd;
use plane_core::{LabelEntry, StateEntry};
use serde_json::Value;
use uuid::Uuid;

/// Messages from background threads (API worker, update check, skill check).
pub enum UiMsg {
    /// `None` means up to date, no `gh`, or check failed (silent).
    UpdateCheck(Option<UpdateHint>),
    SkillCheck(skill_cmd::SkillCheck),
    /// First work-items screen for a project: includes catalogs for columns + preview.
    WorkItemsReady {
        items: Vec<Value>,
        next_cursor: Option<String>,
        states: Vec<StateEntry>,
        labels: Vec<LabelEntry>,
    },
    WorkItemsPage {
        items: Vec<Value>,
        next_cursor: Option<String>,
        append: bool,
    },
    ModulesPage {
        modules: Vec<Value>,
    },
    ModuleIssuesPage {
        items: Vec<Value>,
        next_cursor: Option<String>,
        append: bool,
    },
    Detail {
        issue: Value,
        comments: String,
        project_id: Uuid,
        issue_id: Uuid,
    },
    SearchResults {
        items: Vec<Value>,
    },
    WorkItemUpdated {
        issue: Value,
    },
    WorkItemDeleted {
        issue_id: Uuid,
    },
    ActivityTrend {
        values: Vec<u64>,
        label: String,
    },
}

/// Serialized Plane API jobs (one at a time per worker thread).
pub enum Job {
    OpenProjectWorkItems {
        project_id: Uuid,
    },
    WorkItemsPage {
        project_id: Uuid,
        cursor: Option<String>,
        append: bool,
    },
    LoadModules {
        project_id: Uuid,
    },
    LoadModuleIssues {
        project_id: Uuid,
        module_id: Uuid,
        cursor: Option<String>,
        append: bool,
    },
    LoadDetail {
        project_id: Uuid,
        issue_id: Uuid,
    },
    PostComment {
        project_id: Uuid,
        issue_id: Uuid,
        text: String,
    },
    SearchWorkItems {
        query: String,
        project_id: Option<Uuid>,
        limit: u32,
    },
    PatchWorkItem {
        project_id: Uuid,
        issue_id: Uuid,
        body: Value,
    },
    DeleteWorkItem {
        project_id: Uuid,
        issue_id: Uuid,
    },
    FetchActivityTrend {
        project_id: Uuid,
        issue_id: Uuid,
    },
}
