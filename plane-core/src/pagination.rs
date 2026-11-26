use crate::client::HttpClient;
use crate::error::CoreError;
use serde_json::{json, Map, Value};

pub const MAX_ALL_PAGES: usize = 50;
pub const MAX_ALL_ROWS: usize = 10_000;

#[derive(Debug, Clone, Default)]
pub struct PageParams {
    pub per_page: Option<u32>,
    pub cursor: Option<String>,
    pub fetch_all: bool,
}

impl PageParams {
    /// Every page, 100 rows at a time (capped by `MAX_ALL_PAGES` / `MAX_ALL_ROWS`).
    pub fn all() -> Self {
        Self {
            per_page: Some(100),
            cursor: None,
            fetch_all: true,
        }
    }
}

/// Some CE 1.4.2 list endpoints answer with a bare array instead of the paginated object
/// (e.g. `cycles/?cycle_view=current`). Wrap it so every list has the same shape.
pub fn normalize_list(v: Value) -> Value {
    match v {
        Value::Array(arr) => {
            let n = arr.len();
            json!({
                "results": arr,
                "count": n,
                "total_results": n,
                "next_page_results": false,
            })
        }
        other => other,
    }
}

pub fn merge_page_query(params: &PageParams) -> Vec<(String, String)> {
    let mut q = Vec::new();
    if let Some(n) = params.per_page {
        q.push(("per_page".into(), n.to_string()));
    }
    if let Some(ref c) = params.cursor {
        q.push(("cursor".into(), c.clone()));
    }
    q
}

/// Fetches one page from a paginated GET. Returns merged `results`, next_cursor, truncated.
pub fn fetch_page(
    client: &HttpClient,
    method: &str,
    path: &str,
    extra_query: &[(String, String)],
    params: &PageParams,
) -> Result<(Value, Option<String>, bool), CoreError> {
    if params.fetch_all {
        return fetch_all_pages(client, method, path, extra_query, params);
    }
    let mut q: Vec<(String, String)> = extra_query.to_vec();
    q.extend(merge_page_query(params));
    let v = normalize_list(client.expect_json(method, path, &q, 200..=299)?);
    let next = v
        .get("next_cursor")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    Ok((v, next, false))
}

fn fetch_all_pages(
    client: &HttpClient,
    method: &str,
    path: &str,
    extra_query: &[(String, String)],
    params: &PageParams,
) -> Result<(Value, Option<String>, bool), CoreError> {
    let mut all_results = Vec::new();
    let mut cursor: Option<String> = params.cursor.clone();
    let mut truncated = false;
    let mut pages = 0usize;
    let mut total_rows = 0usize;

    loop {
        if pages >= MAX_ALL_PAGES || total_rows >= MAX_ALL_ROWS {
            truncated = true;
            break;
        }
        let mut q: Vec<(String, String)> = extra_query.to_vec();
        if let Some(n) = params.per_page {
            q.push(("per_page".into(), n.to_string()));
        }
        if let Some(ref c) = cursor {
            q.push(("cursor".into(), c.clone()));
        }
        let v = normalize_list(client.expect_json(method, path, &q, 200..=299)?);
        if let Some(arr) = v.get("results").and_then(|x| x.as_array()) {
            for item in arr {
                if total_rows >= MAX_ALL_ROWS {
                    truncated = true;
                    break;
                }
                all_results.push(item.clone());
                total_rows += 1;
            }
        }
        pages += 1;
        let next = v
            .get("next_cursor")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let more = v
            .get("next_page_results")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if !more || next.is_none() {
            cursor = next;
            break;
        }
        cursor = next;
    }

    let mut out = Map::new();
    out.insert("results".into(), Value::Array(all_results));
    if let Some(c) = cursor {
        out.insert("next_cursor".into(), json!(c));
    }
    out.insert("next_page_results".into(), json!(false));
    Ok((Value::Object(out), None, truncated))
}

#[cfg(test)]
mod tests {
    use super::normalize_list;
    use serde_json::json;

    #[test]
    fn bare_arrays_become_paginated_shape() {
        let v = normalize_list(json!([{"id": "a"}, {"id": "b"}]));
        assert_eq!(v["results"].as_array().unwrap().len(), 2);
        assert_eq!(v["count"], 2);
        assert_eq!(v["next_page_results"], false);
        let page = json!({"results": [], "count": 0});
        assert_eq!(normalize_list(page.clone()), page);
    }
}
