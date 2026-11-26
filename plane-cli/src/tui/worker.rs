//! Serialized Plane API worker (one in-flight job at a time).

use crate::dispatch::policy_gate;
use crate::telemetry;
use crate::tui::format::work_items_from_search_response;
use crate::tui::msg::{Job, UiMsg};
use crate::tui::project_open::load_project_work_list_open;
use plane_core::{Config, CoreError, PageParams, PlaneApi};
use std::sync::mpsc;
use std::thread;
use uuid::Uuid;

/// Returns the job queue sender; results are pushed to `ui_tx` (same channel as update/skill checks).
pub fn spawn_api_worker(
    cfg: Config,
    ui_tx: mpsc::Sender<Result<UiMsg, String>>,
) -> mpsc::Sender<Job> {
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    thread::spawn(move || {
        while let Ok(job) = job_rx.recv() {
            let res = run_job(&cfg, job);
            match res {
                // A policy refusal is an expected outcome, not a fault.
                Err(ref msg) if msg.starts_with(policy_gate::TUI_POLICY_PREFIX) => {}
                Err(_) => telemetry::capture_tui_worker(),
                Ok(_) => {}
            }
            let _ = ui_tx.send(res);
        }
    });
    job_tx
}

fn run_job(cfg: &Config, job: Job) -> Result<UiMsg, String> {
    let mut api = PlaneApi::new(cfg.clone()).map_err(|e| e.to_string())?;
    match job {
        Job::OpenProjectWorkItems { project_id } => open_project_work_items(&mut api, project_id),
        Job::WorkItemsPage {
            project_id,
            cursor,
            append,
        } => work_items_page(&mut api, project_id, cursor, append),
        Job::LoadModules { project_id } => load_modules(&mut api, project_id),
        Job::LoadModuleIssues {
            project_id,
            module_id,
            cursor,
            append,
        } => load_module_issues(&mut api, project_id, module_id, cursor, append),
        Job::LoadDetail {
            project_id,
            issue_id,
        } => load_detail(&mut api, project_id, issue_id),
        Job::PostComment {
            project_id,
            issue_id,
            text,
        } => post_comment(&mut api, project_id, issue_id, text),
        Job::SearchWorkItems {
            query,
            project_id,
            limit,
        } => search_work_items(&api, query, project_id, limit),
        Job::PatchWorkItem {
            project_id,
            issue_id,
            body,
        } => patch_work_item(&mut api, project_id, issue_id, body),
        Job::DeleteWorkItem {
            project_id,
            issue_id,
        } => delete_work_item(&mut api, project_id, issue_id),
        Job::FetchActivityTrend {
            project_id,
            issue_id,
        } => fetch_activity_trend(&api, project_id, issue_id),
    }
}

fn open_project_work_items(api: &mut PlaneApi, project_id: Uuid) -> Result<UiMsg, String> {
    let open =
        load_project_work_list_open(api, project_id).map_err(|e: CoreError| e.to_string())?;
    Ok(UiMsg::WorkItemsReady {
        items: open.items,
        next_cursor: open.next_cursor,
        states: open.states,
        labels: open.labels,
    })
}

fn work_items_page(
    api: &mut PlaneApi,
    project_id: Uuid,
    cursor: Option<String>,
    append: bool,
) -> Result<UiMsg, String> {
    let page = PageParams {
        per_page: Some(50),
        cursor,
        fetch_all: false,
    };
    let (v, next_cursor, _) = api
        .list_work_items(&project_id, &page)
        .map_err(|e| e.to_string())?;
    let items = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(UiMsg::WorkItemsPage {
        items,
        next_cursor,
        append,
    })
}

fn load_modules(api: &mut PlaneApi, project_id: Uuid) -> Result<UiMsg, String> {
    let page = PageParams {
        per_page: Some(100),
        cursor: None,
        fetch_all: false,
    };
    let (v, _, _) = api
        .list_modules(&project_id, &page)
        .map_err(|e| e.to_string())?;
    let modules = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(UiMsg::ModulesPage { modules })
}

fn load_module_issues(
    api: &mut PlaneApi,
    project_id: Uuid,
    module_id: Uuid,
    cursor: Option<String>,
    append: bool,
) -> Result<UiMsg, String> {
    let page = PageParams {
        per_page: Some(50),
        cursor,
        fetch_all: false,
    };
    let (v, next_cursor, _) = api
        .list_module_work_items(&project_id, &module_id, &page)
        .map_err(|e| e.to_string())?;
    let items = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(UiMsg::ModuleIssuesPage {
        items,
        next_cursor,
        append,
    })
}

fn load_detail(api: &mut PlaneApi, project_id: Uuid, issue_id: Uuid) -> Result<UiMsg, String> {
    let issue = api
        .get_work_item(&project_id, &issue_id)
        .map_err(|e| e.to_string())?;
    let page = PageParams {
        per_page: Some(100),
        cursor: None,
        fetch_all: false,
    };
    let (cval, _, _) = api
        .list_comments(&project_id, &issue_id, &page)
        .map_err(|e| e.to_string())?;
    let comments = crate::tui::format::format_comments(&cval);
    Ok(UiMsg::Detail {
        issue,
        comments,
        project_id,
        issue_id,
    })
}

fn post_comment(
    api: &mut PlaneApi,
    project_id: Uuid,
    issue_id: Uuid,
    text: String,
) -> Result<UiMsg, String> {
    let body = serde_json::json!({ "comment_html": text });
    api.create_comment(&project_id, &issue_id, &body)
        .map_err(|e| e.to_string())?;
    load_detail(api, project_id, issue_id)
}

fn search_work_items(
    api: &PlaneApi,
    query: String,
    project_id: Option<Uuid>,
    limit: u32,
) -> Result<UiMsg, String> {
    let v = api
        .search_work_items(&query, project_id.as_ref(), Some(limit))
        .map_err(|e| e.to_string())?;
    let items = work_items_from_search_response(&v);
    Ok(UiMsg::SearchResults { items })
}

fn patch_work_item(
    api: &mut PlaneApi,
    project_id: Uuid,
    issue_id: Uuid,
    body: serde_json::Value,
) -> Result<UiMsg, String> {
    // The project's write policy applies to TUI edits too; refused edits are not sent.
    policy_gate::tui_check_update(api, &project_id, &issue_id, &body)?;
    let issue = api
        .update_work_item(&project_id, &issue_id, &body)
        .map_err(|e| e.to_string())?;
    Ok(UiMsg::WorkItemUpdated { issue })
}

fn delete_work_item(api: &mut PlaneApi, project_id: Uuid, issue_id: Uuid) -> Result<UiMsg, String> {
    policy_gate::tui_check_delete(api, &project_id)?;
    api.delete_work_item(&project_id, &issue_id)
        .map_err(|e| e.to_string())?;
    Ok(UiMsg::WorkItemDeleted { issue_id })
}

fn fetch_activity_trend(api: &PlaneApi, project_id: Uuid, issue_id: Uuid) -> Result<UiMsg, String> {
    let mut all = Vec::new();
    let mut cursor = None;
    for _ in 0..12 {
        let page = PageParams {
            per_page: Some(100),
            cursor: cursor.clone(),
            fetch_all: false,
        };
        let (v, next, _) = api
            .list_activities(&project_id, &issue_id, &page)
            .map_err(|e| e.to_string())?;
        let arr = v
            .get("results")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        if arr.is_empty() && cursor.is_none() {
            break;
        }
        all.extend(arr);
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    let (values, label) = crate::tui::charts::activity_buckets_from_results(&all, 14);
    Ok(UiMsg::ActivityTrend { values, label })
}
