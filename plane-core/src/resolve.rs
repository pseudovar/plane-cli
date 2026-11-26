use crate::catalog::{LabelEntry, ProjectEntry, StateEntry};
use crate::error::CoreError;
use regex::Regex;
use std::sync::OnceLock;
use uuid::Uuid;

fn composite_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Za-z0-9_-]+)-(\d+)$").expect("regex"))
}

/// Returns `(project_identifier, sequence_id)` for workspace work-item path.
pub fn parse_composite_work_item(s: &str) -> Option<(String, u64)> {
    let c = composite_re().captures(s.trim())?;
    let proj = c.get(1)?.as_str().to_string();
    let num: u64 = c.get(2)?.as_str().parse().ok()?;
    Some((proj, num))
}

pub fn resolve_project_id(arg: &str, catalog: &[ProjectEntry]) -> Result<Uuid, CoreError> {
    let arg = arg.trim();
    if let Ok(u) = Uuid::parse_str(arg) {
        return Ok(u);
    }
    if catalog.is_empty() {
        return Err(CoreError::Resolve(
            "project catalog empty; run `plane-cli projects list` or use a project UUID".into(),
        ));
    }
    let matches: Vec<&ProjectEntry> = catalog
        .iter()
        .filter(|p| p.identifier.eq_ignore_ascii_case(arg) || p.name.eq_ignore_ascii_case(arg))
        .collect();
    match matches.len() {
        0 => {
            let sample: Vec<String> = catalog
                .iter()
                .take(8)
                .map(|p| format!("{} ({})", p.identifier, p.name))
                .collect();
            Err(CoreError::Resolve(format!(
                "unknown project {arg:?}; try --refresh. Examples: {}",
                sample.join(", ")
            )))
        }
        1 => Ok(matches[0].id),
        _ => Err(CoreError::Resolve(format!(
            "ambiguous project {arg:?}: {} matches",
            matches.len()
        ))),
    }
}

pub fn resolve_work_item_uuid(arg: &str) -> Result<Uuid, CoreError> {
    Uuid::parse_str(arg.trim()).map_err(|_| {
        CoreError::Resolve(format!(
            "work item {arg:?} is not a UUID; use PROJ-123 style or pass composite to get command"
        ))
    })
}

pub fn resolve_state_id(arg: &str, catalog: &[StateEntry]) -> Result<Uuid, CoreError> {
    let arg = arg.trim();
    if catalog.is_empty() {
        return Err(CoreError::Resolve(
            "state catalog empty for this project; use a state UUID or run with --refresh".into(),
        ));
    }
    let matches: Vec<&StateEntry> = catalog
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(arg))
        .collect();
    match matches.len() {
        0 => {
            let sample: Vec<String> = catalog.iter().take(8).map(|s| s.name.clone()).collect();
            Err(CoreError::Resolve(format!(
                "unknown state {arg:?}; try --refresh. Examples: {}",
                sample.join(", ")
            )))
        }
        1 => Ok(matches[0].id),
        _ => Err(CoreError::Resolve(format!(
            "ambiguous state {arg:?}: {} matches",
            matches.len()
        ))),
    }
}

pub fn resolve_label_id(arg: &str, catalog: &[LabelEntry]) -> Result<Uuid, CoreError> {
    let arg = arg.trim();
    if catalog.is_empty() {
        return Err(CoreError::Resolve(
            "label catalog empty for this project; use label UUIDs or run with --refresh".into(),
        ));
    }
    let matches: Vec<&LabelEntry> = catalog
        .iter()
        .filter(|l| l.name.eq_ignore_ascii_case(arg))
        .collect();
    match matches.len() {
        0 => {
            let sample: Vec<String> = catalog.iter().take(8).map(|l| l.name.clone()).collect();
            Err(CoreError::Resolve(format!(
                "unknown label {arg:?}; try --refresh. Examples: {}",
                sample.join(", ")
            )))
        }
        1 => Ok(matches[0].id),
        _ => Err(CoreError::Resolve(format!(
            "ambiguous label {arg:?}: {} matches",
            matches.len()
        ))),
    }
}
