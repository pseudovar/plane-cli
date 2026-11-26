use crate::catalog::{LabelEntry, StateEntry};
use crate::error::CoreError;
use crate::resolve::{resolve_label_id, resolve_state_id};
use serde_json::{json, Map, Value};
use uuid::Uuid;

/// Coerce `state` / `labels` using project catalogs (names → UUIDs, UUIDs must exist when catalogs non-empty).
pub fn normalize_work_item_body(
    body: Value,
    states: &[StateEntry],
    labels: &[LabelEntry],
) -> Result<Value, CoreError> {
    let mut obj: Map<String, Value> = match body {
        Value::Object(m) => m,
        _ => {
            return Err(CoreError::Usage(
                "work item body must be a JSON object".into(),
            ))
        }
    };

    if let Some(sv) = obj.get("state") {
        let u = resolve_state_json_value(sv, states)?;
        obj.insert("state".into(), json!(u.to_string()));
    }

    if let Some(lv) = obj.get("labels") {
        let arr = lv.as_array().ok_or_else(|| {
            CoreError::Usage("labels must be a JSON array of ids or names".into())
        })?;
        let mut out: Vec<Value> = Vec::new();
        for item in arr {
            let u = resolve_label_json_value(item, labels)?;
            out.push(json!(u.to_string()));
        }
        obj.insert("labels".into(), Value::Array(out));
    }

    if let Some(av) = obj.get("assignees") {
        let arr = av.as_array().ok_or_else(|| {
            CoreError::Usage("assignees must be a JSON array of UUID strings".into())
        })?;
        let mut out = Vec::new();
        for item in arr {
            let u = match item {
                Value::String(s) => Uuid::parse_str(s)
                    .map_err(|_| CoreError::Usage(format!("invalid assignee UUID: {s}")))?,
                _ => {
                    return Err(CoreError::Usage(
                        "each assignee must be a UUID string".into(),
                    ));
                }
            };
            out.push(json!(u.to_string()));
        }
        obj.insert("assignees".into(), Value::Array(out));
    }

    Ok(Value::Object(obj))
}

fn resolve_state_json_value(v: &Value, states: &[StateEntry]) -> Result<Uuid, CoreError> {
    match v {
        Value::String(s) => {
            if let Ok(u) = Uuid::parse_str(s) {
                if states.iter().any(|st| st.id == u) {
                    return Ok(u);
                }
                if states.is_empty() {
                    return Ok(u);
                }
                return Err(CoreError::Resolve(format!(
                    "state UUID {s} not in cached project states; try --refresh"
                )));
            }
            resolve_state_id(s, states)
        }
        Value::Number(n) => {
            let s = n.to_string();
            Uuid::parse_str(&s).map_err(|_| {
                CoreError::Usage(format!("invalid state value (expected UUID or name): {n}"))
            })
        }
        _ => Err(CoreError::Usage(
            "state must be a string (UUID or state name) or omitted".into(),
        )),
    }
}

fn resolve_label_json_value(v: &Value, labels: &[LabelEntry]) -> Result<Uuid, CoreError> {
    match v {
        Value::String(s) => {
            if let Ok(u) = Uuid::parse_str(s) {
                if labels.iter().any(|lb| lb.id == u) {
                    return Ok(u);
                }
                if labels.is_empty() {
                    return Ok(u);
                }
                return Err(CoreError::Resolve(format!(
                    "label UUID {s} not in cached project labels; try --refresh"
                )));
            }
            resolve_label_id(s, labels)
        }
        Value::Number(n) => {
            let s = n.to_string();
            Uuid::parse_str(&s).map_err(|_| {
                CoreError::Usage(format!("invalid label value (expected UUID or name): {n}"))
            })
        }
        _ => Err(CoreError::Usage(
            "each label must be a string (UUID or label name)".into(),
        )),
    }
}
