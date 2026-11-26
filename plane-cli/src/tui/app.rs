use crate::self_cmd::UpdateHint;
use crate::skill_cmd;
use plane_core::ProjectEntry;
use plane_core::{LabelEntry, StateEntry};
use ratatui::widgets::TableState;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulesOrigin {
    ProjectList,
    WorkItemList,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Projects,
    WorkItems,
    Modules,
    ModuleIssues,
    Detail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignOption {
    Clear,
    User { id: Uuid, label: String },
}

#[derive(Debug)]
pub enum Modal {
    StatePick {
        idx: usize,
    },
    PriorityPick {
        idx: usize,
    },
    TitleEdit {
        draft: String,
    },
    DescEdit {
        draft: String,
    },
    DeleteConfirm,
    AssignPick {
        options: Vec<AssignOption>,
        idx: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpPane {
    Context,
    Global,
}

/// Snapshot of normal browse before applying search results.
#[derive(Clone, Debug)]
pub struct WorkItemsBrowseBackup {
    pub pages: Vec<Vec<Value>>,
    pub page_idx: usize,
    pub next_cursor: Option<String>,
    pub wi_idx: usize,
}

pub struct App {
    pub theme: crate::tui::theme::Theme,
    pub screen: Screen,
    pub projects: Vec<ProjectEntry>,
    pub proj_idx: usize,
    /// Loaded pages of work items (see `wi_page_idx`).
    pub wi_pages: Vec<Vec<Value>>,
    pub wi_page_idx: usize,
    pub wi_next_cursor: Option<String>,
    pub wi_idx: usize,
    pub wi_table: TableState,
    /// Workflow states for the project whose work items are shown.
    pub wi_states: Vec<StateEntry>,
    pub wi_labels: Vec<LabelEntry>,
    pub modules: Vec<Value>,
    pub module_idx: usize,
    pub module_issues_pages: Vec<Vec<Value>>,
    pub module_issues_page_idx: usize,
    pub module_issues_next_cursor: Option<String>,
    pub module_issues_idx: usize,
    pub module_issues_table: TableState,
    pub active_module_id: Option<Uuid>,
    pub active_module_name: Option<String>,
    /// Where `Screen::Modules` was entered from (controls `b`).
    pub modules_origin: ModulesOrigin,
    pub detail: Option<Value>,
    pub detail_project: Option<Uuid>,
    pub detail_issue: Option<Uuid>,
    pub comments_preview: String,
    pub comments_scroll: u16,
    pub comment_draft: Option<String>,
    /// `/` on work items: type query, Enter runs search, Esc cancels draft.
    pub search_draft: Option<String>,
    /// When editing search, search whole workspace vs current project only.
    pub search_workspace_wide: bool,
    /// True when the table shows search hits (Esc restores browse if backup exists).
    pub wi_search_active: bool,
    pub wi_browse_backup: Option<WorkItemsBrowseBackup>,
    pub help_overlay: Option<HelpPane>,
    /// When true, work-items screen shows a state distribution strip (loaded pages only).
    pub chart_distribution: bool,
    /// Normalized sparkline heights (0–100) for focused item activity (best-effort).
    pub activity_trend: Option<Vec<u64>>,
    pub activity_label: Option<String>,
    pub modal: Option<Modal>,
    pub status: String,
    pub update_hint: Option<UpdateHint>,
    pub skill_check: Option<skill_cmd::SkillCheck>,
}

impl App {
    pub fn new(projects: Vec<ProjectEntry>) -> Self {
        Self {
            theme: crate::tui::theme::Theme::from_env(),
            screen: Screen::Projects,
            projects,
            proj_idx: 0,
            wi_pages: Vec::new(),
            wi_page_idx: 0,
            wi_next_cursor: None,
            wi_idx: 0,
            wi_table: {
                let mut t = TableState::default();
                t.select(Some(0));
                t
            },
            wi_states: Vec::new(),
            wi_labels: Vec::new(),
            modules: Vec::new(),
            module_idx: 0,
            module_issues_pages: Vec::new(),
            module_issues_page_idx: 0,
            module_issues_next_cursor: None,
            module_issues_idx: 0,
            module_issues_table: {
                let mut t = TableState::default();
                t.select(Some(0));
                t
            },
            active_module_id: None,
            active_module_name: None,
            modules_origin: ModulesOrigin::ProjectList,
            detail: None,
            detail_project: None,
            detail_issue: None,
            comments_preview: String::new(),
            comments_scroll: 0,
            comment_draft: None,
            search_draft: None,
            search_workspace_wide: false,
            wi_search_active: false,
            wi_browse_backup: None,
            help_overlay: None,
            chart_distribution: false,
            activity_trend: None,
            activity_label: None,
            modal: None,
            status: "j/k move · Enter open work items · m modules · s setup env · b back · n/p pages · r refresh · ? help · q quit"
                .into(),
            update_hint: None,
            skill_check: None,
        }
    }

    /// Clear search mode. Returns `true` if the caller should reload the project work list
    /// (no browse snapshot existed, e.g. search from an empty list).
    pub fn clear_search_results(&mut self) -> bool {
        if !self.wi_search_active {
            return false;
        }
        self.wi_search_active = false;
        if let Some(b) = self.wi_browse_backup.take() {
            self.wi_pages = b.pages;
            self.wi_page_idx = b.page_idx;
            self.wi_next_cursor = b.next_cursor;
            self.wi_idx = b.wi_idx;
            self.sync_wi_table();
            false
        } else {
            self.wi_pages.clear();
            self.wi_page_idx = 0;
            self.wi_next_cursor = None;
            self.wi_idx = 0;
            self.sync_wi_table();
            true
        }
    }

    pub fn sync_wi_table(&mut self) {
        let page = self
            .wi_pages
            .get(self.wi_page_idx)
            .map(|p| p.as_slice())
            .unwrap_or(&[]);
        if page.is_empty() {
            self.wi_table.select(None);
            return;
        }
        let i = self.wi_idx.min(page.len().saturating_sub(1));
        self.wi_idx = i;
        self.wi_table.select(Some(i));
    }

    pub fn sync_module_issues_table(&mut self) {
        let page = self
            .module_issues_pages
            .get(self.module_issues_page_idx)
            .map(|p| p.as_slice())
            .unwrap_or(&[]);
        if page.is_empty() {
            self.module_issues_table.select(None);
            return;
        }
        let i = self.module_issues_idx.min(page.len().saturating_sub(1));
        self.module_issues_idx = i;
        self.module_issues_table.select(Some(i));
    }
}
