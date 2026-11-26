//! Sorted completion projections for shell tab-completion (`plane-cli complete …`).

use plane_core::{
    label_record, state_record, states_sorted, CoreError, LabelEntry, ProjectEntry, StateEntry,
};
use serde_json::Value;
use uuid::Uuid;

pub fn sorted_project_identifiers(catalog: &[ProjectEntry]) -> Vec<String> {
    let mut ids: Vec<String> = catalog
        .iter()
        .map(|p| p.identifier.clone())
        .filter(|s| !s.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

pub fn sorted_state_names(states: &[StateEntry]) -> Vec<String> {
    let mut names: Vec<String> = states.iter().map(|s| s.name.clone()).collect();
    names.sort();
    names.dedup();
    names
}

/// Rich JSON rows for `complete states --json` (keeps legacy `names` at call site).
pub fn state_records_json(states: &[StateEntry]) -> Vec<Value> {
    states_sorted(states)
        .into_iter()
        .map(state_record)
        .collect()
}

fn labels_sorted(labels: &[LabelEntry]) -> Vec<&LabelEntry> {
    let mut out: Vec<&LabelEntry> = labels.iter().collect();
    out.sort_by(|a, b| {
        let a_key = a.sort_order.unwrap_or(f64::MAX);
        let b_key = b.sort_order.unwrap_or(f64::MAX);
        a_key
            .partial_cmp(&b_key)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            })
    });
    out
}

/// Rich JSON rows for `complete labels --json`.
pub fn label_records_json(labels: &[LabelEntry]) -> Vec<Value> {
    labels_sorted(labels)
        .into_iter()
        .map(label_record)
        .collect()
}

pub fn sorted_label_names(labels: &[LabelEntry]) -> Vec<String> {
    let mut names: Vec<String> = labels.iter().map(|l| l.name.clone()).collect();
    names.sort();
    names.dedup();
    names
}

/// Parses module list JSON (`results[].id` UUIDs), sorts and deduplicates.
pub fn sorted_module_uuid_strings(v: &Value) -> Result<Vec<String>, CoreError> {
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| CoreError::Usage("modules response missing results array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::Usage("module entry missing id".into()))?;
        out.push(id);
    }
    out.sort();
    out.dedup();
    Ok(out.into_iter().map(|u| u.to_string()).collect())
}
