//! Modal pickers and destructive confirm (mutations α–δ).

use super::app::{App, AssignOption, Modal, Screen};
use super::format::state_id_from_work_item;
use super::msg::Job;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use std::sync::mpsc;
use uuid::Uuid;

pub const PRIORITIES: &[&str] = &["none", "low", "medium", "high", "urgent"];

pub fn focused_issue(app: &App) -> Option<&serde_json::Value> {
    match app.screen {
        Screen::WorkItems => app
            .wi_pages
            .get(app.wi_page_idx)
            .and_then(|p| p.get(app.wi_idx)),
        Screen::ModuleIssues => app
            .module_issues_pages
            .get(app.module_issues_page_idx)
            .and_then(|p| p.get(app.module_issues_idx)),
        Screen::Detail => app.detail.as_ref(),
        _ => None,
    }
}

pub fn open_state_pick(app: &mut App) -> bool {
    let Some(wi) = focused_issue(app) else {
        return false;
    };
    if app.wi_states.is_empty() {
        return false;
    }
    let cur = state_id_from_work_item(wi);
    let idx = cur
        .and_then(|id| app.wi_states.iter().position(|s| s.id == id))
        .unwrap_or(0);
    app.modal = Some(Modal::StatePick { idx });
    true
}

pub fn open_priority_pick(app: &mut App) -> bool {
    let Some(wi) = focused_issue(app) else {
        return false;
    };
    let cur = wi
        .get("priority")
        .and_then(|x| x.as_str())
        .unwrap_or("none");
    let idx = PRIORITIES.iter().position(|p| *p == cur).unwrap_or(0);
    app.modal = Some(Modal::PriorityPick { idx });
    true
}

pub fn open_title_edit(app: &mut App) -> bool {
    let Some(wi) = focused_issue(app) else {
        return false;
    };
    let name = wi
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    app.modal = Some(Modal::TitleEdit { draft: name });
    true
}

pub fn open_desc_edit(app: &mut App) -> bool {
    let Some(wi) = focused_issue(app) else {
        return false;
    };
    let raw = wi
        .get("description_html")
        .or_else(|| wi.get("description"))
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let desc: String = crate::tui::format::html_to_plain_display(raw, 80)
        .chars()
        .take(4000)
        .collect();
    app.modal = Some(Modal::DescEdit { draft: desc });
    true
}

pub fn collect_assign_options(app: &App) -> Vec<AssignOption> {
    let mut out = vec![AssignOption::Clear];
    let mut seen = std::collections::HashSet::new();
    for wi in app.wi_pages.iter().flatten() {
        if let Some(arr) = wi.get("assignees").and_then(|a| a.as_array()) {
            for a in arr {
                let id = a
                    .get("id")
                    .and_then(|x| x.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());
                let label = a
                    .get("display_name")
                    .and_then(|x| x.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        let f = a.get("first_name").and_then(|x| x.as_str()).unwrap_or("");
                        let l = a.get("last_name").and_then(|x| x.as_str()).unwrap_or("");
                        let c = format!("{f} {l}").trim().to_string();
                        if c.is_empty() {
                            a.get("email").and_then(|x| x.as_str()).map(String::from)
                        } else {
                            Some(c)
                        }
                    });
                if let (Some(id), Some(lbl)) = (id, label) {
                    if seen.insert(id) {
                        out.push(AssignOption::User { id, label: lbl });
                    }
                }
            }
        }
    }
    out
}

pub fn open_assign_pick(app: &mut App) -> bool {
    let opts = collect_assign_options(app);
    if opts.len() <= 1 {
        return false;
    }
    let idx = focused_issue(app)
        .and_then(|wi| wi.get("assignees").and_then(|a| a.as_array()))
        .and_then(|arr| arr.first())
        .and_then(|a| {
            a.get("id")
                .and_then(|x| x.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
        })
        .and_then(|id| {
            opts.iter()
                .position(|o| matches!(o, AssignOption::User { id: uid, .. } if *uid == id))
        })
        .unwrap_or(1)
        .min(opts.len().saturating_sub(1));
    app.modal = Some(Modal::AssignPick { options: opts, idx });
    true
}

pub fn open_delete_confirm(app: &mut App) -> bool {
    if focused_issue(app).is_none() {
        return false;
    }
    app.modal = Some(Modal::DeleteConfirm);
    true
}

/// Returns `true` if the key was consumed by modal handling.
pub fn handle_modal_key(app: &mut App, key: &KeyEvent, job_tx: &mpsc::Sender<Job>) -> bool {
    if app.modal.is_none() {
        return false;
    }
    let Some(pid) = app.projects.get(app.proj_idx).map(|p| p.id) else {
        app.modal = None;
        return true;
    };
    let iid = match focused_issue(app).and_then(|wi| {
        wi.get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    }) {
        Some(id) => id,
        None => {
            app.modal = None;
            return true;
        }
    };

    match app.modal.as_mut() {
        None => return true,
        Some(Modal::StatePick { idx }) => match key.code {
            KeyCode::Esc => {
                app.modal = None;
            }
            KeyCode::Enter => {
                let Some(st) = app.wi_states.get(*idx) else {
                    app.modal = None;
                    return true;
                };
                let body = json!({ "state": st.id.to_string() });
                let _ = job_tx.send(Job::PatchWorkItem {
                    project_id: pid,
                    issue_id: iid,
                    body,
                });
                app.status = "saving state…".into();
                app.modal = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                *idx = (*idx + 1).min(app.wi_states.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *idx = idx.saturating_sub(1);
            }
            _ => {}
        },
        Some(Modal::PriorityPick { idx }) => match key.code {
            KeyCode::Esc => {
                app.modal = None;
            }
            KeyCode::Enter => {
                let pr = PRIORITIES.get(*idx).unwrap_or(&"none");
                let body = json!({ "priority": pr });
                let _ = job_tx.send(Job::PatchWorkItem {
                    project_id: pid,
                    issue_id: iid,
                    body,
                });
                app.status = "saving priority…".into();
                app.modal = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                *idx = (*idx + 1).min(PRIORITIES.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *idx = idx.saturating_sub(1);
            }
            _ => {}
        },
        Some(Modal::TitleEdit { draft }) => match key.code {
            KeyCode::Esc => {
                app.modal = None;
            }
            KeyCode::Enter => {
                let text = std::mem::take(draft);
                let body = json!({ "name": text });
                let _ = job_tx.send(Job::PatchWorkItem {
                    project_id: pid,
                    issue_id: iid,
                    body,
                });
                app.status = "saving title…".into();
                app.modal = None;
            }
            KeyCode::Backspace => {
                draft.pop();
            }
            KeyCode::Char(c) if !c.is_control() => {
                draft.push(c);
            }
            _ => {}
        },
        Some(Modal::DescEdit { draft }) => match key.code {
            KeyCode::Esc => {
                app.modal = None;
            }
            KeyCode::Enter => {
                let text = std::mem::take(draft);
                let body = json!({ "description": text });
                let _ = job_tx.send(Job::PatchWorkItem {
                    project_id: pid,
                    issue_id: iid,
                    body,
                });
                app.status = "saving description…".into();
                app.modal = None;
            }
            KeyCode::Backspace => {
                draft.pop();
            }
            KeyCode::Char(c) if !c.is_control() => {
                draft.push(c);
            }
            _ => {}
        },
        Some(Modal::DeleteConfirm) => match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.modal = None;
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let _ = job_tx.send(Job::DeleteWorkItem {
                    project_id: pid,
                    issue_id: iid,
                });
                app.status = "deleting…".into();
                app.modal = None;
            }
            _ => {}
        },
        Some(Modal::AssignPick { options, idx }) => match key.code {
            KeyCode::Esc => {
                app.modal = None;
            }
            KeyCode::Enter => {
                let body = match options.get(*idx) {
                    Some(AssignOption::Clear) => json!({ "assignees": [] }),
                    Some(AssignOption::User { id, .. }) => {
                        json!({ "assignees": vec![id.to_string()] })
                    }
                    None => json!({}),
                };
                let _ = job_tx.send(Job::PatchWorkItem {
                    project_id: pid,
                    issue_id: iid,
                    body,
                });
                app.status = "saving assignees…".into();
                app.modal = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                *idx = (*idx + 1).min(options.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *idx = idx.saturating_sub(1);
            }
            _ => {}
        },
    }
    true
}

/// Shift+D opens delete confirm when a work item is focused.
pub fn wants_delete_confirm(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT) && matches!(key.code, KeyCode::Char('D'))
}
