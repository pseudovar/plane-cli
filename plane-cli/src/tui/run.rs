use super::app::{App, HelpPane, ModulesOrigin, Screen, WorkItemsBrowseBackup};
use super::modal;
use super::msg::{Job, UiMsg};
use super::view;
use super::wizards;
use super::worker::spawn_api_worker;
use crate::tui::format;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use plane_core::{CoreError, PlaneApi};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

fn sync_search_draft_status(app: &mut App) {
    let scope = if app.search_workspace_wide {
        "workspace"
    } else {
        "project"
    };
    let d = app.search_draft.as_deref().unwrap_or("");
    app.status = format!("search ({scope}): {d} · Enter run · Esc cancel · Ctrl-G toggle scope");
}

pub fn run_tui(api: &mut PlaneApi) -> Result<(), CoreError> {
    enable_raw_mode().map_err(|e| CoreError::Usage(format!("raw mode: {e}")))?;
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| CoreError::Usage(format!("alternate screen: {e}")))?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))
        .map_err(|e| CoreError::Usage(format!("terminal: {e}")))?;
    let result = run_loop(&mut terminal, api);
    drop(terminal);
    let _ = execute!(stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    api: &mut PlaneApi,
) -> Result<(), CoreError> {
    let projects = api.fetch_projects_catalog(false)?;
    let mut app = App::new(projects);

    let (ui_tx, ui_rx) = mpsc::channel::<Result<UiMsg, String>>();
    let cfg = api.config.clone();
    let job_tx = spawn_api_worker(cfg.clone(), ui_tx.clone());

    let tx_release = ui_tx.clone();
    thread::spawn(move || {
        let hint = crate::self_cmd::try_update_hint();
        let _ = tx_release.send(Ok(UiMsg::UpdateCheck(hint)));
    });
    let tx_skill = ui_tx.clone();
    thread::spawn(move || {
        let check = crate::skill_cmd::check_installed();
        let _ = tx_skill.send(Ok(UiMsg::SkillCheck(check)));
    });

    loop {
        terminal
            .draw(|f| view::render(f, &mut app))
            .map_err(|e| CoreError::Usage(format!("terminal draw: {e}")))?;

        if event::poll(Duration::from_millis(200))
            .map_err(|e| CoreError::Usage(format!("poll: {e}")))?
        {
            if let Event::Key(key) =
                event::read().map_err(|e| CoreError::Usage(format!("read event: {e}")))?
            {
                if key.kind == KeyEventKind::Release {
                    continue;
                }

                if let Some(ref mut draft) = app.comment_draft {
                    match key.code {
                        KeyCode::Esc => {
                            app.comment_draft = None;
                            app.status = "detail".into();
                        }
                        KeyCode::Enter => {
                            let text = std::mem::take(draft);
                            app.comment_draft = None;
                            if let (Some(pid), Some(iid)) = (app.detail_project, app.detail_issue) {
                                let _ = job_tx.send(Job::PostComment {
                                    project_id: pid,
                                    issue_id: iid,
                                    text,
                                });
                                app.status = "posting comment…".into();
                            }
                        }
                        KeyCode::Backspace => {
                            draft.pop();
                        }
                        KeyCode::Char(c) => {
                            draft.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                if let Some(ref mut draft) = app.search_draft {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('g') | KeyCode::Char('G'))
                    {
                        app.search_workspace_wide = !app.search_workspace_wide;
                        sync_search_draft_status(&mut app);
                        continue;
                    }
                    match key.code {
                        KeyCode::Esc => {
                            app.search_draft = None;
                            app.status = view::work_items_footer(&app);
                        }
                        KeyCode::Enter => {
                            let q = draft.trim().to_string();
                            if q.is_empty() {
                                app.status = "empty search query".into();
                                continue;
                            }
                            let text = std::mem::take(draft);
                            app.search_draft = None;
                            let pid = app.projects[app.proj_idx].id;
                            let project_id = if app.search_workspace_wide {
                                None
                            } else {
                                Some(pid)
                            };
                            let _ = job_tx.send(Job::SearchWorkItems {
                                query: text,
                                project_id,
                                limit: 50,
                            });
                            app.status = "searching…".into();
                        }
                        KeyCode::Backspace => {
                            draft.pop();
                            sync_search_draft_status(&mut app);
                        }
                        KeyCode::Char(c) if !c.is_control() => {
                            draft.push(c);
                            sync_search_draft_status(&mut app);
                        }
                        _ => {}
                    }
                    continue;
                }

                if let Some(pane) = app.help_overlay {
                    match key.code {
                        KeyCode::Esc => {
                            app.help_overlay = None;
                        }
                        KeyCode::Char('?') => {
                            app.help_overlay = match pane {
                                HelpPane::Context => Some(HelpPane::Global),
                                HelpPane::Global => None,
                            };
                        }
                        KeyCode::Char('a') => {
                            app.help_overlay = Some(HelpPane::Global);
                        }
                        KeyCode::Char('q') => return Ok(()),
                        _ => {}
                    }
                    continue;
                }

                if modal::handle_modal_key(&mut app, &key, &job_tx) {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => {
                        if app.screen == Screen::WorkItems && app.wi_search_active {
                            let need_reload = app.clear_search_results();
                            if need_reload {
                                let pid = app.projects[app.proj_idx].id;
                                let _ = job_tx.send(Job::OpenProjectWorkItems { project_id: pid });
                                app.status = "reloading work items…".into();
                            } else {
                                app.status = view::work_items_footer(&app);
                            }
                            continue;
                        }
                        return Ok(());
                    }
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('?') => {
                        app.help_overlay = Some(HelpPane::Context);
                    }
                    KeyCode::Char('/') => {
                        if matches!(app.screen, Screen::WorkItems) {
                            app.search_draft = Some(String::new());
                            app.search_workspace_wide = false;
                            sync_search_draft_status(&mut app);
                        }
                    }
                    KeyCode::Char('v') if matches!(app.screen, Screen::WorkItems) => {
                        app.chart_distribution = !app.chart_distribution;
                        app.status = if app.chart_distribution {
                            format!("distribution strip on · {}", view::work_items_footer(&app))
                        } else {
                            format!("distribution strip off · {}", view::work_items_footer(&app))
                        };
                    }
                    KeyCode::Char('S')
                        if key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(
                                app.screen,
                                Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                            )
                            && modal::open_state_pick(&mut app) =>
                    {
                        app.status = "state · j/k · Enter apply · Esc cancel".into();
                    }
                    KeyCode::Char('P')
                        if key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(
                                app.screen,
                                Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                            )
                            && modal::open_priority_pick(&mut app) =>
                    {
                        app.status = "priority · j/k · Enter apply · Esc cancel".into();
                    }
                    KeyCode::Char('A')
                        if key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(
                                app.screen,
                                Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                            ) =>
                    {
                        if modal::open_assign_pick(&mut app) {
                            app.status = "assignee · j/k · Enter apply · Esc cancel".into();
                        } else {
                            app.status = "assignee: load work items with assignees first".into();
                        }
                    }
                    KeyCode::Char('G')
                        if key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(
                                app.screen,
                                Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                            ) =>
                    {
                        let pid = app.projects.get(app.proj_idx).map(|p| p.id);
                        let iid = modal::focused_issue(&app).and_then(|wi| {
                            wi.get("id")
                                .and_then(|x| x.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok())
                        });
                        if let (Some(pid), Some(iid)) = (pid, iid) {
                            let _ = job_tx.send(Job::FetchActivityTrend {
                                project_id: pid,
                                issue_id: iid,
                            });
                            app.status = "loading activity trend…".into();
                        }
                    }
                    KeyCode::Char('e')
                        if matches!(
                            app.screen,
                            Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                        ) && modal::open_title_edit(&mut app) =>
                    {
                        app.status = "title · type · Enter save · Esc cancel".into();
                    }
                    KeyCode::Char('w')
                        if matches!(
                            app.screen,
                            Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                        ) && modal::open_desc_edit(&mut app) =>
                    {
                        app.status = "description · type · Enter save · Esc cancel".into();
                    }
                    _ if modal::wants_delete_confirm(&key)
                        && matches!(
                            app.screen,
                            Screen::WorkItems | Screen::ModuleIssues | Screen::Detail
                        )
                        && modal::open_delete_confirm(&mut app) =>
                    {
                        app.status = "delete? y / n · Esc cancel".into();
                    }
                    KeyCode::Char('r') => {
                        app.projects = api.fetch_projects_catalog(true)?;
                        app.status = "refreshed projects cache".into();
                    }
                    KeyCode::Char('j') | KeyCode::Down => match app.screen {
                        Screen::Projects if !app.projects.is_empty() => {
                            app.proj_idx = (app.proj_idx + 1).min(app.projects.len() - 1);
                        }
                        Screen::WorkItems => {
                            let page = app
                                .wi_pages
                                .get(app.wi_page_idx)
                                .map(|p| p.as_slice())
                                .unwrap_or(&[]);
                            if !page.is_empty() {
                                app.wi_idx = (app.wi_idx + 1).min(page.len() - 1);
                                app.sync_wi_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                            }
                        }
                        Screen::Modules if !app.modules.is_empty() => {
                            app.module_idx = (app.module_idx + 1).min(app.modules.len() - 1);
                        }
                        Screen::ModuleIssues => {
                            let page = app
                                .module_issues_pages
                                .get(app.module_issues_page_idx)
                                .map(|p| p.as_slice())
                                .unwrap_or(&[]);
                            if !page.is_empty() {
                                app.module_issues_idx =
                                    (app.module_issues_idx + 1).min(page.len() - 1);
                                app.sync_module_issues_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Char('k') | KeyCode::Up => match app.screen {
                        Screen::Projects => {
                            app.proj_idx = app.proj_idx.saturating_sub(1);
                        }
                        Screen::WorkItems => {
                            app.wi_idx = app.wi_idx.saturating_sub(1);
                            app.sync_wi_table();
                            app.activity_trend = None;
                            app.activity_label = None;
                        }
                        Screen::Modules => {
                            app.module_idx = app.module_idx.saturating_sub(1);
                        }
                        Screen::ModuleIssues => {
                            app.module_issues_idx = app.module_issues_idx.saturating_sub(1);
                            app.sync_module_issues_table();
                            app.activity_trend = None;
                            app.activity_label = None;
                        }
                        _ => {}
                    },
                    KeyCode::Char('n') => {
                        if let Screen::WorkItems = app.screen {
                            if app.wi_search_active {
                                app.status = "search is single-page · Esc restore list".into();
                                continue;
                            }
                            if app.wi_page_idx + 1 < app.wi_pages.len() {
                                app.wi_page_idx += 1;
                                let page_len = app
                                    .wi_pages
                                    .get(app.wi_page_idx)
                                    .map(|p| p.len())
                                    .unwrap_or(0);
                                app.wi_idx = app.wi_idx.min(page_len.saturating_sub(1));
                                app.sync_wi_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                                app.status = view::work_items_footer(&app);
                            } else if let Some(cur) = app.wi_next_cursor.clone() {
                                app.activity_trend = None;
                                app.activity_label = None;
                                let pid = app.projects[app.proj_idx].id;
                                let _ = job_tx.send(Job::WorkItemsPage {
                                    project_id: pid,
                                    cursor: Some(cur),
                                    append: true,
                                });
                                app.status = "loading next page…".into();
                            } else {
                                app.status = "last page".into();
                            }
                        }
                        if let Screen::ModuleIssues = app.screen {
                            if app.module_issues_page_idx + 1 < app.module_issues_pages.len() {
                                app.module_issues_page_idx += 1;
                                let page_len = app
                                    .module_issues_pages
                                    .get(app.module_issues_page_idx)
                                    .map(|p| p.len())
                                    .unwrap_or(0);
                                app.module_issues_idx =
                                    app.module_issues_idx.min(page_len.saturating_sub(1));
                                app.sync_module_issues_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                                app.status = view::module_issues_footer(&app);
                            } else if let (Some(cur), Some(mid)) =
                                (app.module_issues_next_cursor.clone(), app.active_module_id)
                            {
                                app.activity_trend = None;
                                app.activity_label = None;
                                let pid = app.projects[app.proj_idx].id;
                                let _ = job_tx.send(Job::LoadModuleIssues {
                                    project_id: pid,
                                    module_id: mid,
                                    cursor: Some(cur),
                                    append: true,
                                });
                                app.status = "loading next module issues page…".into();
                            } else {
                                app.status = "last module issues page".into();
                            }
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Screen::WorkItems = app.screen {
                            if app.wi_search_active {
                                app.status = "search is single-page · Esc restore list".into();
                                continue;
                            }
                            if app.wi_page_idx > 0 {
                                app.wi_page_idx -= 1;
                                let page_len = app
                                    .wi_pages
                                    .get(app.wi_page_idx)
                                    .map(|p| p.len())
                                    .unwrap_or(0);
                                app.wi_idx = app.wi_idx.min(page_len.saturating_sub(1));
                                app.sync_wi_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                                app.status = view::work_items_footer(&app);
                            }
                        }
                        if let Screen::ModuleIssues = app.screen {
                            if app.module_issues_page_idx > 0 {
                                app.module_issues_page_idx -= 1;
                                let page_len = app
                                    .module_issues_pages
                                    .get(app.module_issues_page_idx)
                                    .map(|p| p.len())
                                    .unwrap_or(0);
                                app.module_issues_idx =
                                    app.module_issues_idx.min(page_len.saturating_sub(1));
                                app.sync_module_issues_table();
                                app.activity_trend = None;
                                app.activity_label = None;
                                app.status = view::module_issues_footer(&app);
                            }
                        }
                    }
                    KeyCode::PageUp | KeyCode::Char('[') => {
                        if let Screen::Detail = app.screen {
                            app.comments_scroll = app.comments_scroll.saturating_sub(4);
                        }
                    }
                    KeyCode::PageDown | KeyCode::Char(']') => {
                        if let Screen::Detail = app.screen {
                            app.comments_scroll = app.comments_scroll.saturating_add(4);
                        }
                    }
                    KeyCode::Char('c') => {
                        if matches!(app.screen, Screen::Detail) {
                            app.comment_draft = Some(String::new());
                            app.status =
                                "comment draft: type text · Enter send · Esc cancel".into();
                        }
                    }
                    KeyCode::Enter => match app.screen {
                        Screen::Projects => {
                            if app.projects.is_empty() {
                                continue;
                            }
                            let pid = app.projects[app.proj_idx].id;
                            let _ = job_tx.send(Job::OpenProjectWorkItems { project_id: pid });
                            app.status = "loading work items…".into();
                        }
                        Screen::WorkItems => {
                            let page = app
                                .wi_pages
                                .get(app.wi_page_idx)
                                .map(|p| p.as_slice())
                                .unwrap_or(&[]);
                            if page.is_empty() {
                                continue;
                            }
                            let pid = app.projects[app.proj_idx].id;
                            let item = &page[app.wi_idx];
                            let iid = item
                                .get("id")
                                .and_then(|x| x.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            if let Some(iid) = iid {
                                let _ = job_tx.send(Job::LoadDetail {
                                    project_id: pid,
                                    issue_id: iid,
                                });
                                app.status = "loading detail…".into();
                            }
                        }
                        Screen::Modules => {
                            if app.modules.is_empty() {
                                continue;
                            }
                            let pid = app.projects[app.proj_idx].id;
                            let module = &app.modules[app.module_idx];
                            let mid = module
                                .get("id")
                                .and_then(|x| x.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            let module_name = module
                                .get("name")
                                .and_then(|x| x.as_str())
                                .map(str::to_string);
                            if let Some(mid) = mid {
                                app.active_module_id = Some(mid);
                                app.active_module_name = module_name;
                                let _ = job_tx.send(Job::LoadModuleIssues {
                                    project_id: pid,
                                    module_id: mid,
                                    cursor: None,
                                    append: false,
                                });
                                app.status = "loading module issues…".into();
                            }
                        }
                        Screen::ModuleIssues => {
                            let page = app
                                .module_issues_pages
                                .get(app.module_issues_page_idx)
                                .map(|p| p.as_slice())
                                .unwrap_or(&[]);
                            if page.is_empty() {
                                continue;
                            }
                            let pid = app.projects[app.proj_idx].id;
                            let item = &page[app.module_issues_idx];
                            let iid = item
                                .get("id")
                                .and_then(|x| x.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            if let Some(iid) = iid {
                                let _ = job_tx.send(Job::LoadDetail {
                                    project_id: pid,
                                    issue_id: iid,
                                });
                                app.status = "loading detail…".into();
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Char('m') => match app.screen {
                        Screen::Projects => {
                            if app.projects.is_empty() {
                                continue;
                            }
                            app.modules_origin = ModulesOrigin::ProjectList;
                            let pid = app.projects[app.proj_idx].id;
                            let _ = job_tx.send(Job::LoadModules { project_id: pid });
                            app.status = "loading modules…".into();
                        }
                        Screen::WorkItems => {
                            if app.projects.is_empty() {
                                continue;
                            }
                            app.modules_origin = ModulesOrigin::WorkItemList;
                            let pid = app.projects[app.proj_idx].id;
                            let _ = job_tx.send(Job::LoadModules { project_id: pid });
                            app.status = "loading modules…".into();
                        }
                        _ => {}
                    },
                    KeyCode::Char('i') => {
                        if let Some(ref check) = app.skill_check {
                            if check.any_complete {
                                app.status = "plane-cli skill already installed".into();
                                continue;
                            }
                        }
                        app.status = "running skill installer wizard…".into();
                        match wizards::run_skill_install_wizard(terminal) {
                            Ok(()) => {
                                let check = crate::skill_cmd::check_installed();
                                let installed = check.any_complete;
                                app.skill_check = Some(check);
                                app.status = if installed {
                                    "skill installed".into()
                                } else {
                                    "installer finished; skill still not detected".into()
                                };
                            }
                            Err(e) => {
                                app.status = format!("skill install failed: {e}");
                            }
                        }
                    }
                    KeyCode::Char('s') => {
                        app.status = "running env setup wizard…".into();
                        match wizards::run_config_init_wizard(terminal) {
                            Ok(path) => {
                                app.status = format!(
                                    "wrote {} · run with --env-file (or PLANE_DOTENV)",
                                    path.display()
                                );
                            }
                            Err(e) => {
                                app.status = format!("config setup failed: {e}");
                            }
                        }
                    }
                    KeyCode::Char('b') => match app.screen {
                        Screen::Detail => {
                            if !app.module_issues_pages.is_empty() {
                                app.screen = Screen::ModuleIssues;
                                app.status = view::module_issues_footer(&app);
                            } else {
                                app.screen = Screen::WorkItems;
                                app.status = view::work_items_footer(&app);
                            }
                            app.detail = None;
                            app.detail_project = None;
                            app.detail_issue = None;
                            app.comments_preview.clear();
                            app.comments_scroll = 0;
                        }
                        Screen::WorkItems => {
                            app.screen = Screen::Projects;
                            app.wi_pages.clear();
                            app.wi_page_idx = 0;
                            app.wi_next_cursor = None;
                            app.wi_idx = 0;
                            app.wi_states.clear();
                            app.wi_labels.clear();
                            app.wi_search_active = false;
                            app.wi_browse_backup = None;
                            app.search_draft = None;
                            app.status = view::default_project_footer();
                        }
                        Screen::ModuleIssues => {
                            app.screen = Screen::Modules;
                            app.module_issues_pages.clear();
                            app.module_issues_page_idx = 0;
                            app.module_issues_next_cursor = None;
                            app.module_issues_idx = 0;
                            app.active_module_id = None;
                            app.active_module_name = None;
                            app.status = view::modules_list_footer(&app);
                        }
                        Screen::Modules => match app.modules_origin {
                            ModulesOrigin::ProjectList => {
                                app.screen = Screen::Projects;
                                app.modules.clear();
                                app.module_idx = 0;
                                app.status = view::default_project_footer();
                            }
                            ModulesOrigin::WorkItemList => {
                                app.screen = Screen::WorkItems;
                                app.modules.clear();
                                app.module_idx = 0;
                                app.status = view::work_items_footer(&app);
                            }
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                Ok(UiMsg::UpdateCheck(hint)) => {
                    app.update_hint = hint;
                }
                Ok(UiMsg::SkillCheck(check)) => {
                    if !check.any_complete {
                        app.status = "skill not found · press i to run installer wizard".into();
                    }
                    app.skill_check = Some(check);
                }
                Ok(UiMsg::WorkItemsReady {
                    items,
                    next_cursor,
                    states,
                    labels,
                }) => {
                    app.wi_pages = vec![items];
                    app.wi_page_idx = 0;
                    app.wi_next_cursor = next_cursor;
                    app.wi_idx = 0;
                    app.wi_states = states;
                    app.wi_labels = labels;
                    app.screen = Screen::WorkItems;
                    app.wi_search_active = false;
                    app.wi_browse_backup = None;
                    app.search_draft = None;
                    app.activity_trend = None;
                    app.activity_label = None;
                    app.sync_wi_table();
                    app.status = view::work_items_footer(&app);
                }
                Ok(UiMsg::WorkItemsPage {
                    items,
                    next_cursor,
                    append,
                }) => {
                    if append {
                        app.wi_pages.push(items);
                        app.wi_page_idx = app.wi_pages.len().saturating_sub(1);
                    } else {
                        app.wi_pages = vec![items];
                        app.wi_page_idx = 0;
                    }
                    app.wi_next_cursor = next_cursor;
                    app.wi_idx = 0;
                    app.screen = Screen::WorkItems;
                    app.wi_search_active = false;
                    app.wi_browse_backup = None;
                    app.search_draft = None;
                    app.activity_trend = None;
                    app.activity_label = None;
                    app.sync_wi_table();
                    app.status = view::work_items_footer(&app);
                }
                Ok(UiMsg::SearchResults { items }) => {
                    if app.screen == Screen::WorkItems && !app.wi_search_active {
                        app.wi_browse_backup = Some(WorkItemsBrowseBackup {
                            pages: app.wi_pages.clone(),
                            page_idx: app.wi_page_idx,
                            next_cursor: app.wi_next_cursor.clone(),
                            wi_idx: app.wi_idx,
                        });
                    }
                    app.wi_pages = vec![items];
                    app.wi_page_idx = 0;
                    app.wi_next_cursor = None;
                    app.wi_idx = 0;
                    app.wi_search_active = true;
                    app.screen = Screen::WorkItems;
                    app.activity_trend = None;
                    app.activity_label = None;
                    app.sync_wi_table();
                    app.status = view::work_items_footer(&app);
                }
                Ok(UiMsg::ModulesPage { modules }) => {
                    app.modules = modules;
                    app.module_idx = 0;
                    app.screen = Screen::Modules;
                    app.status = format!(
                        "modules · Enter open module issues · b → {} · q quit",
                        view::modules_back_label(app.modules_origin)
                    );
                }
                Ok(UiMsg::ModuleIssuesPage {
                    items,
                    next_cursor,
                    append,
                }) => {
                    if append {
                        app.module_issues_pages.push(items);
                        app.module_issues_page_idx =
                            app.module_issues_pages.len().saturating_sub(1);
                    } else {
                        app.module_issues_pages = vec![items];
                        app.module_issues_page_idx = 0;
                    }
                    app.module_issues_next_cursor = next_cursor;
                    app.module_issues_idx = 0;
                    app.screen = Screen::ModuleIssues;
                    app.activity_trend = None;
                    app.activity_label = None;
                    app.sync_module_issues_table();
                    app.status = view::module_issues_footer(&app);
                }
                Ok(UiMsg::Detail {
                    issue,
                    comments,
                    project_id,
                    issue_id,
                }) => {
                    app.detail = Some(issue);
                    app.detail_project = Some(project_id);
                    app.detail_issue = Some(issue_id);
                    app.comments_preview = comments;
                    app.comments_scroll = 0;
                    app.screen = Screen::Detail;
                    app.status =
                        "detail · PgUp/PgDn or [/] scroll comments · c comment · b back".into();
                }
                Ok(UiMsg::WorkItemUpdated { issue }) => {
                    format::merge_work_item_into_pages(&mut app.wi_pages[..], &issue);
                    format::merge_work_item_into_pages(&mut app.module_issues_pages[..], &issue);
                    if let Some(ref d) = app.detail {
                        if d.get("id") == issue.get("id") {
                            app.detail = Some(issue.clone());
                        }
                    }
                    app.sync_wi_table();
                    app.sync_module_issues_table();
                    app.status = "saved".into();
                }
                Ok(UiMsg::WorkItemDeleted { issue_id }) => {
                    format::remove_work_item_from_pages(&mut app.wi_pages, &issue_id);
                    format::remove_work_item_from_pages(&mut app.module_issues_pages, &issue_id);
                    if app.detail_issue == Some(issue_id) {
                        app.detail = None;
                        app.detail_project = None;
                        app.detail_issue = None;
                        app.comments_preview.clear();
                        app.comments_scroll = 0;
                        app.screen = Screen::WorkItems;
                    }
                    app.sync_wi_table();
                    app.sync_module_issues_table();
                    app.status = "deleted".into();
                }
                Ok(UiMsg::ActivityTrend { values, label }) => {
                    app.activity_trend = Some(values);
                    app.activity_label = Some(label);
                    app.status = view::work_items_footer(&app);
                }
                Err(e) if e.starts_with(crate::dispatch::policy_gate::TUI_POLICY_PREFIX) => {
                    app.status = format!("{e} · nothing was changed");
                }
                Err(e) => {
                    app.status = format!("error: {e} · prior data kept · retry the same action");
                }
            }
        }
    }
}
