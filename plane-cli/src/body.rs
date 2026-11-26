use plane_core::{load_json_source, merge_objects, CoreError};
use serde_json::{json, Value};

fn normalize_work_item_description_key(body: Value) -> Value {
    let Value::Object(mut map) = body else {
        return body;
    };
    if map.contains_key("description_html") {
        map.remove("description");
        return Value::Object(map);
    }
    if let Some(description) = map.remove("description") {
        map.insert("description_html".into(), description);
    }
    Value::Object(map)
}

pub fn merge_body(json_path: Option<&str>, patch: Value) -> Result<Value, CoreError> {
    let base = load_json_source(json_path)?.unwrap_or(json!({}));
    Ok(merge_objects(base, patch))
}

pub fn project_create_flags(
    json_path: Option<&str>,
    name: Option<String>,
    description: Option<String>,
    identifier: Option<String>,
    project_lead: Option<String>,
) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(n) = name {
        p.insert("name".into(), json!(n));
    }
    if let Some(d) = description {
        p.insert("description".into(), json!(d));
    }
    if let Some(i) = identifier {
        p.insert("identifier".into(), json!(i));
    }
    if let Some(l) = project_lead {
        p.insert("project_lead".into(), json!(l));
    }
    merge_body(json_path, Value::Object(p))
}

pub fn work_item_flags(
    json_path: Option<&str>,
    name: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    state: Option<String>,
    assignees: Option<Vec<String>>,
) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(n) = name {
        p.insert("name".into(), json!(n));
    }
    if let Some(d) = description {
        p.insert("description_html".into(), json!(d));
    }
    if let Some(pr) = priority {
        p.insert("priority".into(), json!(pr));
    }
    if let Some(s) = state {
        p.insert("state".into(), json!(s));
    }
    if let Some(ids) = assignees {
        p.insert(
            "assignees".into(),
            Value::Array(ids.into_iter().map(|s| json!(s)).collect()),
        );
    }
    Ok(normalize_work_item_description_key(merge_body(
        json_path,
        Value::Object(p),
    )?))
}

pub fn comment_flags(
    json_path: Option<&str>,
    comment_html: Option<String>,
) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(c) = comment_html {
        p.insert("comment_html".into(), json!(c));
    }
    merge_body(json_path, Value::Object(p))
}

pub fn link_flags(
    json_path: Option<&str>,
    url: Option<String>,
    title: Option<String>,
) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(u) = url {
        p.insert("url".into(), json!(u));
    }
    if let Some(t) = title {
        p.insert("title".into(), json!(t));
    }
    merge_body(json_path, Value::Object(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn work_item_flags_emit_description_html_for_flag() {
        let out = work_item_flags(
            None,
            Some("Title".into()),
            Some("Knowledge resource".into()),
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(out["name"], json!("Title"));
        assert_eq!(out["description_html"], json!("Knowledge resource"));
        assert!(out.get("description").is_none());
    }

    #[test]
    fn work_item_flags_translate_body_description_to_description_html() {
        let file = NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{"description":"Body description","priority":"high"}"#,
        )
        .unwrap();

        let out = work_item_flags(
            Some(file.path().to_str().unwrap()),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(out["description_html"], json!("Body description"));
        assert_eq!(out["priority"], json!("high"));
        assert!(out.get("description").is_none());
    }
}
