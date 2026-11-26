//! Full-screen layout and widgets.

use crate::tui::app::{App, AssignOption, HelpPane, Modal, ModulesOrigin, Screen};
use crate::tui::format::{self, format_work_item_preview, module_issues_page_status, page_status};
use crate::tui::help_text;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Sparkline, Table,
};
use ratatui::Frame;
use serde_json::Value;

/// Show split master–detail when the terminal is large enough.
const PREVIEW_MIN_WIDTH: u16 = 100;
const PREVIEW_MIN_HEIGHT: u16 = 22;

fn use_preview_split(area: Rect) -> bool {
    area.width >= PREVIEW_MIN_WIDTH && area.height >= PREVIEW_MIN_HEIGHT
}

fn work_item_rows(app: &App, page: &[Value], selected_idx: usize) -> Vec<Row<'static>> {
    page.iter()
        .enumerate()
        .map(|(i, wi)| {
            let row_style = if i == selected_idx {
                app.theme.selected
            } else {
                app.theme.normal
            };
            let seq = wi
                .get("sequence_id")
                .and_then(|x| x.as_u64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let name = wi.get("name").and_then(|x| x.as_str()).unwrap_or("?");
            let (st, g) = format::state_display_token(wi, &app.wi_states);
            let st_style = if i == selected_idx {
                app.theme.selected
            } else {
                app.theme.state_style(g.as_deref())
            };
            let pr = format::priority_display(wi);
            let asg = format::assignee_display(wi);
            Row::new(vec![
                Cell::from(seq).style(row_style),
                Cell::from(truncate_static(name, 48)).style(row_style),
                Cell::from(st).style(st_style),
                Cell::from(pr).style(row_style),
                Cell::from(asg).style(row_style),
            ])
        })
        .collect()
}

fn truncate_static(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let end = t
        .char_indices()
        .map(|(i, _)| i)
        .nth(max.saturating_sub(1))
        .unwrap_or(t.len());
    format!("{}…", &t[..end])
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let mut foot_lines: u16 = 1;
    if app.update_hint.is_some() {
        foot_lines += 1;
    }
    if matches!(&app.skill_check, Some(c) if !c.any_complete) {
        foot_lines += 1;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(foot_lines)])
        .split(frame.area());

    match app.screen {
        Screen::Projects => render_projects(frame, app, chunks[0]),
        Screen::WorkItems => render_work_items_screen(frame, app, chunks[0]),
        Screen::Modules => render_modules(frame, app, chunks[0]),
        Screen::ModuleIssues => render_module_issues_screen(frame, app, chunks[0]),
        Screen::Detail => render_detail(frame, app, chunks[0]),
    }

    render_footer(frame, app, chunks[1]);

    if let Some(ref m) = app.modal {
        render_modal_overlay(frame, app, frame.area(), m);
    }

    if let Some(pane) = app.help_overlay {
        render_help_overlay(frame, app, frame.area(), pane);
    }
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &App, area: Rect, pane: HelpPane) {
    let w = (area.width * 4 / 5).max(44).min(area.width);
    let h = (area.height * 4 / 5).max(16).min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w, h);
    frame.render_widget(Clear, popup);
    let (title, body) = match pane {
        HelpPane::Context => (
            "Help (this screen) — ? all keys · a all keys · Esc close",
            help_text::context(app.screen),
        ),
        HelpPane::Global => ("Help (all keys) — ? or Esc close", help_text::GLOBAL),
    };
    let p = Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(app.theme.status),
        )
        .style(Style::default().bg(Color::Black));
    frame.render_widget(p, popup);
}

fn render_modal_overlay(frame: &mut Frame<'_>, app: &App, area: Rect, m: &Modal) {
    let w = (area.width * 4 / 5).max(44).min(area.width);
    let h = (area.height * 4 / 5).max(12).min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w, h);
    frame.render_widget(Clear, popup);

    let block_title: &str = match m {
        Modal::StatePick { .. } => "State (Enter apply · j/k · Esc)",
        Modal::PriorityPick { .. } => "Priority (Enter apply · j/k · Esc)",
        Modal::TitleEdit { .. } => "Title (Enter save · Esc cancel)",
        Modal::DescEdit { .. } => "Description (Enter save · Esc cancel)",
        Modal::DeleteConfirm => "Delete work item? (y confirm · n/Esc cancel)",
        Modal::AssignPick { .. } => "Assignee (Enter apply · j/k · Esc)",
    };

    match m {
        Modal::StatePick { idx } => {
            let items: Vec<ListItem<'static>> = app
                .wi_states
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let style = if i == *idx {
                        app.theme.selected
                    } else {
                        app.theme.normal
                    };
                    ListItem::new(s.name.clone()).style(style)
                })
                .collect();
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(block_title)
                    .border_style(app.theme.status),
            );
            frame.render_widget(list, popup);
        }
        Modal::PriorityPick { idx } => {
            let items: Vec<ListItem<'static>> = crate::tui::modal::PRIORITIES
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let style = if i == *idx {
                        app.theme.selected
                    } else {
                        app.theme.normal
                    };
                    ListItem::new((*p).to_string()).style(style)
                })
                .collect();
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(block_title)
                    .border_style(app.theme.status),
            );
            frame.render_widget(list, popup);
        }
        Modal::AssignPick { options, idx } => {
            let items: Vec<ListItem<'static>> = options
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let style = if i == *idx {
                        app.theme.selected
                    } else {
                        app.theme.normal
                    };
                    let line = match o {
                        AssignOption::Clear => "(clear assignees)".to_string(),
                        AssignOption::User { label, .. } => label.clone(),
                    };
                    ListItem::new(line).style(style)
                })
                .collect();
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(block_title)
                    .border_style(app.theme.status),
            );
            frame.render_widget(list, popup);
        }
        Modal::TitleEdit { draft } | Modal::DescEdit { draft } => {
            let p = Paragraph::new(draft.as_str())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(block_title)
                        .border_style(app.theme.status),
                )
                .style(Style::default().bg(Color::Black));
            frame.render_widget(p, popup);
        }
        Modal::DeleteConfirm => {
            let p =
                Paragraph::new("This cannot be undone.\n\nPress y to delete, n or Esc to cancel.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(block_title)
                            .border_style(app.theme.accent),
                    )
                    .style(Style::default().bg(Color::Black));
            frame.render_widget(p, popup);
        }
    }
}

fn render_projects(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == app.proj_idx {
                app.theme.selected
            } else {
                app.theme.normal
            };
            let line = format!("{} — {}", p.identifier, p.name);
            ListItem::new(line).style(style)
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Projects"));
    frame.render_widget(list, area);
}

fn render_work_items_screen(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let page_owned: Vec<Value> = app
        .wi_pages
        .get(app.wi_page_idx)
        .cloned()
        .unwrap_or_default();
    let idx = app.wi_idx;
    let title = format!(
        "Work items {} · page {}/{}",
        app.projects[app.proj_idx].identifier,
        app.wi_page_idx + 1,
        app.wi_pages.len().max(1)
    );

    let show_dist = app.chart_distribution;
    let show_spark = app
        .activity_trend
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let body_area = if show_dist || show_spark {
        let mut cons: Vec<Constraint> = Vec::new();
        if show_dist {
            cons.push(Constraint::Length(3));
        }
        if show_spark {
            cons.push(Constraint::Length(3));
        }
        cons.push(Constraint::Min(3));
        let vchunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(cons)
            .split(area);
        let mut row = 0usize;
        if show_dist {
            let dist = crate::tui::charts::state_distribution(&app.wi_pages, &app.wi_states);
            let line: String = if dist.is_empty() {
                "(no work items on loaded pages)".into()
            } else {
                dist.iter()
                    .take(10)
                    .map(|(n, c)| format!("{n}: {c}"))
                    .collect::<Vec<_>>()
                    .join("  │  ")
            };
            let p = Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("State distribution (loaded pages)"),
            );
            frame.render_widget(p, vchunks[row]);
            row += 1;
        }
        if show_spark {
            let data = app.activity_trend.as_deref().unwrap_or(&[]);
            let spark_title = app
                .activity_label
                .as_deref()
                .unwrap_or("Activity (per day)");
            let spark = Sparkline::default()
                .data(data)
                .block(Block::default().borders(Borders::ALL).title(spark_title));
            frame.render_widget(spark, vchunks[row]);
            row += 1;
        }
        vchunks[row]
    } else {
        area
    };

    if use_preview_split(body_area) && !page_owned.is_empty() {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(body_area);
        let page = page_owned.as_slice();
        render_issue_table(frame, app, split[0], page, idx, &title, true);
        render_preview_pane(frame, app, split[1], page, idx);
    } else {
        render_issue_table(
            frame,
            app,
            body_area,
            page_owned.as_slice(),
            idx,
            &title,
            true,
        );
    }
}

fn render_module_issues_screen(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let page_owned: Vec<Value> = app
        .module_issues_pages
        .get(app.module_issues_page_idx)
        .cloned()
        .unwrap_or_default();
    let idx = app.module_issues_idx;
    let module_name = app.active_module_name.as_deref().unwrap_or("module");
    let title = format!(
        "{module_name} · page {}/{}",
        app.module_issues_page_idx + 1,
        app.module_issues_pages.len().max(1)
    );
    if use_preview_split(area) && !page_owned.is_empty() {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(area);
        let page = page_owned.as_slice();
        render_issue_table(frame, app, split[0], page, idx, &title, false);
        render_preview_pane(frame, app, split[1], page, idx);
    } else {
        render_issue_table(frame, app, area, page_owned.as_slice(), idx, &title, false);
    }
}

fn render_issue_table(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    page: &[Value],
    selected_idx: usize,
    title: &str,
    work_items: bool,
) {
    let header = Row::new(vec![
        Cell::from("#"),
        Cell::from("Title"),
        Cell::from("State"),
        Cell::from("Pri"),
        Cell::from("Assignee"),
    ])
    .style(app.theme.muted.add_modifier(Modifier::BOLD))
    .height(1);

    let rows = work_item_rows(app, page, selected_idx);
    let widths = [
        Constraint::Length(6),
        Constraint::Min(16),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Length(22),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    if work_items {
        frame.render_stateful_widget(table, area, &mut app.wi_table);
    } else {
        frame.render_stateful_widget(table, area, &mut app.module_issues_table);
    }
}

fn render_preview_pane(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    page: &[Value],
    focus_idx: usize,
) {
    let text = page
        .get(focus_idx)
        .map(|wi| format_work_item_preview(wi, &app.wi_states, &app.wi_labels))
        .unwrap_or_default();
    let p = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Preview (metadata)"),
    );
    frame.render_widget(p, area);
}

fn render_modules(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .modules
        .iter()
        .enumerate()
        .map(|(i, module)| {
            let style = if i == app.module_idx {
                app.theme.selected
            } else {
                app.theme.normal
            };
            let name = module.get("name").and_then(|x| x.as_str()).unwrap_or("?");
            ListItem::new(name.to_string()).style(style)
        })
        .collect();
    let title = format!("Modules ({})", app.projects[app.proj_idx].identifier);
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);
}

fn render_detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let left_col_w = inner[0].width.saturating_sub(2) as usize;
    let left_text = if let Some(ref d) = app.detail {
        let name = d.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let desc = d
            .get("description_html")
            .or_else(|| d.get("description"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let plain = format::html_to_plain_display(desc, left_col_w);
        format!("{name}\n\n{plain}")
    } else {
        String::new()
    };
    let left =
        Paragraph::new(left_text).block(Block::default().borders(Borders::ALL).title("Work item"));
    let mut comment_block = Block::default()
        .borders(Borders::ALL)
        .title("Comments (PgUp/PgDn · [/])");
    if let Some(ref draft) = app.comment_draft {
        comment_block = comment_block.title(format!("Draft: {draft}"));
    }
    let right = Paragraph::new(app.comments_preview.as_str())
        .block(comment_block)
        .scroll((0, app.comments_scroll));
    frame.render_widget(left, inner[0]);
    frame.render_widget(right, inner[1]);
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let foot = Layout::default()
        .direction(Direction::Vertical)
        .constraints({
            let mut v = vec![Constraint::Length(1)];
            if app.update_hint.is_some() {
                v.push(Constraint::Length(1));
            }
            if matches!(&app.skill_check, Some(c) if !c.any_complete) {
                v.push(Constraint::Length(1));
            }
            v
        })
        .split(area);

    let bar = Line::from(app.status.as_str());
    let status = Paragraph::new(bar).style(app.theme.status);
    frame.render_widget(status, foot[0]);

    let mut line = 1usize;
    if let Some(ref h) = app.update_hint {
        let tip = format!(
            "↑ New release {} (you have v{}) — {}",
            h.latest_tag, h.current, h.short_tip
        );
        let update_bar = Paragraph::new(tip).style(app.theme.warn);
        frame.render_widget(update_bar, foot[line]);
        line += 1;
    }
    if matches!(&app.skill_check, Some(c) if !c.any_complete) {
        let skill_tip = "Agent skill not detected — press i to run installer wizard";
        let skill_bar = Paragraph::new(skill_tip).style(app.theme.accent);
        frame.render_widget(skill_bar, foot[line]);
    }
}

/// Public so `input` / `run` can build status strings DRY.
pub fn default_project_footer() -> String {
    "j/k move · Enter open work items · m modules · s setup env · b back · n/p pages · r refresh · ? help · q quit"
        .into()
}

pub fn work_items_footer(app: &App) -> String {
    let mut s = page_status(
        app.wi_pages.len(),
        app.wi_page_idx,
        app.wi_next_cursor.as_ref(),
    );
    if app.wi_search_active {
        s.push_str(" · search results · Esc restore list");
    } else {
        s.push_str(" · / search");
    }
    s
}

pub fn module_issues_footer(app: &App) -> String {
    let name = app.active_module_name.as_deref().unwrap_or("module");
    module_issues_page_status(
        app.module_issues_pages.len(),
        app.module_issues_page_idx,
        app.module_issues_next_cursor.as_ref(),
        name,
    )
}

pub fn modules_list_footer(_app: &App) -> String {
    "modules · Enter open module issues · b back".into()
}

/// Restore logical `b` target for modules screen.
pub fn modules_back_label(origin: ModulesOrigin) -> &'static str {
    match origin {
        ModulesOrigin::ProjectList => "projects",
        ModulesOrigin::WorkItemList => "work items",
    }
}
