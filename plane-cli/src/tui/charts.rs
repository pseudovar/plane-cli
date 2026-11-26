//! State distribution from loaded pages; activity bucketing for sparklines (best-effort).

use crate::tui::format::state_id_from_work_item;
use plane_core::StateEntry;
use serde_json::Value;
use std::collections::BTreeMap;

/// Count work items per resolved state name (full name from catalog when possible).
pub fn state_distribution(pages: &[Vec<Value>], states: &[StateEntry]) -> Vec<(String, u64)> {
    let mut m: BTreeMap<String, u64> = BTreeMap::new();
    for wi in pages.iter().flatten() {
        let label = if let Some(id) = state_id_from_work_item(wi) {
            states
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| id.to_string())
        } else if let Some(n) = wi
            .get("state")
            .and_then(|x| x.as_object())
            .and_then(|o| o.get("name"))
            .and_then(|x| x.as_str())
        {
            n.to_string()
        } else {
            "—".into()
        };
        *m.entry(label).or_insert(0) += 1;
    }
    m.into_iter().collect()
}

/// Bucket activity `created_at` strings (ISO-8601) by calendar day; return up to last `max_days` keys sorted.
pub fn activity_buckets_from_results(results: &[Value], max_days: usize) -> (Vec<u64>, String) {
    let mut per_day: BTreeMap<String, u64> = BTreeMap::new();
    for a in results {
        let Some(ts) = a.get("created_at").and_then(|x| x.as_str()) else {
            continue;
        };
        let day = ts.get(..10).unwrap_or(ts).to_string();
        *per_day.entry(day).or_insert(0) += 1;
    }
    let keys: Vec<_> = per_day.keys().cloned().collect();
    let tail: Vec<_> = if keys.len() > max_days {
        keys[keys.len().saturating_sub(max_days)..].to_vec()
    } else {
        keys
    };
    let raw: Vec<u64> = tail.iter().map(|k| *per_day.get(k).unwrap_or(&0)).collect();
    let mx = raw.iter().copied().max().unwrap_or(1);
    let norm: Vec<u64> = raw
        .iter()
        .map(|n| (*n).saturating_mul(100).saturating_div(mx).max(1))
        .collect();
    let label = format!(
        "best-effort · {} activities · {} days",
        results.len(),
        norm.len()
    );
    (norm, label)
}
