//! Shared formatting for work-item JSON (lists, preview, tables).

use plane_core::{LabelEntry, StateEntry};
use serde_json::Value;
use std::io::Cursor;
use uuid::Uuid;

/// Max characters after HTML→text for TUI panes (keeps layout responsive).
const PLAIN_FROM_HTML_MAX_CHARS: usize = 8000;

/// Convert Plane `description_html` / `comment_html` to wrapped plain text for the TUI.
pub fn html_to_plain_display(html: &str, width: usize) -> String {
    let html = html.trim();
    if html.is_empty() {
        return String::new();
    }
    let width = width.max(20);
    let mut s = match html2text::from_read(Cursor::new(html.as_bytes()), width) {
        Ok(t) => t,
        Err(_) => html.chars().filter(|c| !c.is_control()).collect(),
    };
    s = s
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    if s.len() > PLAIN_FROM_HTML_MAX_CHARS {
        let mut end = PLAIN_FROM_HTML_MAX_CHARS;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

/// Resolve state UUID from various Plane API shapes.
pub fn state_id_from_work_item(v: &Value) -> Option<Uuid> {
    match v.get("state") {
        Some(Value::String(s)) => Uuid::parse_str(s).ok(),
        Some(Value::Object(o)) => o
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok()),
        _ => None,
    }
}

fn abbrev_token(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        return t.to_string();
    }
    format!("{}…", &t[..max.saturating_sub(1)])
}

/// Short label for table column + token for accessibility (P+R).
pub fn state_display_token(v: &Value, states: &[StateEntry]) -> (String, Option<String>) {
    if let Some(obj) = v.get("state").and_then(|x| x.as_object()) {
        let name = obj.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let group = obj.get("group").and_then(|x| x.as_str());
        if !name.is_empty() {
            return (abbrev_token(name, 14), group.map(str::to_string));
        }
    }
    if let Some(id) = state_id_from_work_item(v) {
        if let Some(st) = states.iter().find(|x| x.id == id) {
            return (
                abbrev_token(&st.name, 14),
                st.group.clone().map(|g| g.to_ascii_lowercase()),
            );
        }
        let short = id.to_string();
        let short = short.split('-').next().unwrap_or(&short).to_string();
        return (abbrev_token(&short, 14), None);
    }
    ("—".into(), None)
}

pub fn priority_display(v: &Value) -> String {
    match v.get("priority") {
        Some(Value::String(s)) => abbrev_token(s, 6),
        Some(Value::Null) | None => "—".into(),
        Some(other) => abbrev_token(&other.to_string(), 6),
    }
}

pub fn assignee_display(v: &Value) -> String {
    if let Some(arr) = v.get("assignees").and_then(|a| a.as_array()) {
        let mut names = Vec::new();
        for a in arr {
            let n = a
                .get("display_name")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    let f = a.get("first_name").and_then(|x| x.as_str()).unwrap_or("");
                    let l = a.get("last_name").and_then(|x| x.as_str()).unwrap_or("");
                    let combined = format!("{f} {l}");
                    let t = combined.trim();
                    if !t.is_empty() {
                        Some(t.to_string())
                    } else {
                        a.get("email").and_then(|x| x.as_str()).map(String::from)
                    }
                });
            if let Some(n) = n {
                names.push(n);
            }
        }
        if !names.is_empty() {
            let joined = names.join(", ");
            return abbrev_token(&joined, 22);
        }
    }
    if let Some(obj) = v.get("assignee").and_then(|x| x.as_object()) {
        let n = obj
            .get("display_name")
            .and_then(|x| x.as_str())
            .or_else(|| obj.get("email").and_then(|x| x.as_str()));
        if let Some(n) = n {
            return abbrev_token(n, 22);
        }
    }
    "—".into()
}

pub fn labels_preview(v: &Value, catalog: &[LabelEntry]) -> String {
    let mut ids = Vec::new();
    if let Some(arr) = v.get("labels").and_then(|a| a.as_array()) {
        for x in arr {
            if let Some(s) = x.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                ids.push(s);
            }
        }
    }
    if ids.is_empty() {
        return String::new();
    }
    let mut names = Vec::new();
    for id in ids.iter().take(8) {
        let name = catalog
            .iter()
            .find(|l| &l.id == id)
            .map(|l| l.name.as_str())
            .unwrap_or("?");
        names.push(name);
    }
    names.join(", ")
}

pub fn modules_preview_hint(v: &Value) -> Option<String> {
    if let Some(arr) = v.get("issue_module").and_then(|x| x.as_array()) {
        let names: Vec<_> = arr
            .iter()
            .filter_map(|m| m.get("name").and_then(|x| x.as_str()))
            .take(4)
            .collect();
        if !names.is_empty() {
            return Some(names.join(", "));
        }
    }
    if let Some(arr) = v.get("modules").and_then(|x| x.as_array()) {
        let names: Vec<_> = arr
            .iter()
            .filter_map(|m| m.get("name").and_then(|x| x.as_str()))
            .take(4)
            .collect();
        if !names.is_empty() {
            return Some(names.join(", "));
        }
    }
    if let Some(obj) = v.get("module").and_then(|x| x.as_object()) {
        return obj.get("name").and_then(|x| x.as_str()).map(str::to_string);
    }
    None
}

/// Multi-line preview for the master–detail pane.
pub fn format_work_item_preview(v: &Value, states: &[StateEntry], labels: &[LabelEntry]) -> String {
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("?");
    let (st, _) = state_display_token(v, states);
    let pr = priority_display(v);
    let asg = assignee_display(v);
    let labs = labels_preview(v, labels);
    let mods = modules_preview_hint(v);
    let mut out = String::new();
    out.push_str(name);
    out.push_str("\n\n");
    out.push_str("State: ");
    out.push_str(&st);
    out.push_str("  ·  Pri: ");
    out.push_str(&pr);
    out.push_str("\nAssignee: ");
    out.push_str(&asg);
    if !labs.is_empty() {
        out.push_str("\nLabels: ");
        out.push_str(&labs);
    }
    if let Some(m) = mods {
        out.push_str("\nModules: ");
        out.push_str(&m);
    }
    out
}

/// Replace a work item (matched by `id`) inside loaded pages.
pub fn merge_work_item_into_pages(pages: &mut [Vec<Value>], issue: &Value) {
    let Some(new_id) = issue.get("id").and_then(|x| x.as_str()) else {
        return;
    };
    for page in pages.iter_mut() {
        for wi in page.iter_mut() {
            if wi.get("id").and_then(|x| x.as_str()) == Some(new_id) {
                *wi = issue.clone();
                return;
            }
        }
    }
}

/// Remove a work item from all pages (may leave empty inner vectors).
pub fn remove_work_item_from_pages(pages: &mut [Vec<Value>], issue_id: &Uuid) {
    let id = issue_id.to_string();
    for page in pages.iter_mut() {
        page.retain(|wi| wi.get("id").and_then(|x| x.as_str()) != Some(id.as_str()));
    }
}

/// Normalize search API JSON into a list of work-item objects.
pub fn work_items_from_search_response(v: &Value) -> Vec<Value> {
    if let Some(arr) = v.get("results").and_then(|x| x.as_array()) {
        return arr.clone();
    }
    if let Some(arr) = v.as_array() {
        return arr.clone();
    }
    Vec::new()
}

pub fn format_comments(v: &Value) -> String {
    let Some(arr) = v.get("results").and_then(|x| x.as_array()) else {
        return String::new();
    };
    let mut s = String::new();
    for c in arr.iter().take(80) {
        let html = c
            .get("comment_html")
            .or_else(|| c.get("comment"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let stripped = html_to_plain_display(html, 72);
        let excerpt: String = stripped.chars().take(280).collect();
        s.push_str(&excerpt);
        s.push('\n');
    }
    s
}

pub fn page_status(wi_pages: usize, wi_page_idx: usize, wi_next_cursor: Option<&String>) -> String {
    let n = wi_pages;
    let cur = wi_page_idx + 1;
    let more = if wi_next_cursor.is_some() {
        " · more after n"
    } else {
        ""
    };
    format!("work items · page {cur} / {n}{more} · n next · p prev · m modules · b back")
}

pub fn module_issues_page_status(
    module_issues_pages: usize,
    module_issues_page_idx: usize,
    module_issues_next_cursor: Option<&String>,
    module_name: &str,
) -> String {
    let n = module_issues_pages;
    let cur = module_issues_page_idx + 1;
    let more = if module_issues_next_cursor.is_some() {
        " · more after n"
    } else {
        ""
    };
    format!("{module_name} issues · page {cur} / {n}{more} · n next · p prev · b back")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_plain_strips_tags_and_keeps_words() {
        let t = html_to_plain_display("<p>Hello <strong>world</strong></p>", 40);
        assert!(t.contains("Hello"));
        assert!(t.contains("world"));
        assert!(!t.contains('<'));
    }

    #[test]
    fn html_to_plain_list_items() {
        let t = html_to_plain_display("<ul><li>One</li><li>Two</li></ul>", 20);
        assert!(t.contains("One"));
        assert!(t.contains("Two"));
    }
}
