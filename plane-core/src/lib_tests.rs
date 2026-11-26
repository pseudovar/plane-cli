#[cfg(test)]
mod tests {
    use crate::{
        effective_json_output, merge_objects, normalize_work_item_body, parse_composite_work_item,
        projects_from_value, LabelEntry, StateEntry,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn effective_json_output_cli_flag() {
        assert!(effective_json_output(true));
    }

    #[test]
    fn parse_composite() {
        assert_eq!(
            parse_composite_work_item("FOO-42"),
            Some(("FOO".into(), 42))
        );
        assert_eq!(parse_composite_work_item("ab-1"), Some(("ab".into(), 1)));
        assert!(parse_composite_work_item("not-a-composite").is_none());
    }

    #[test]
    fn merge_objects_overrides() {
        let a = json!({"name": "a", "x": 1});
        let b = json!({"name": "b"});
        let m = merge_objects(a, b);
        assert_eq!(m["name"], "b");
        assert_eq!(m["x"], 1);
    }

    #[test]
    fn projects_from_api_shape() {
        let v = json!({
            "results": [
                {"id": "550e8400-e29b-41d4-a716-446655440000", "name": "P", "identifier": "PROJ"}
            ]
        });
        let p = projects_from_value(&v).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].identifier, "PROJ");
    }

    #[test]
    fn normalize_work_item_state_and_labels() {
        let sid = Uuid::nil();
        let lid = Uuid::max();
        let states = vec![StateEntry {
            id: sid,
            name: "Todo".into(),
            group: None,
            color: None,
            sequence: None,
            is_default: None,
        }];
        let labels = vec![LabelEntry {
            id: lid,
            name: "bug".into(),
            color: None,
            description: None,
            sort_order: None,
        }];
        let body = json!({
            "name": "x",
            "state": "Todo",
            "labels": ["bug", lid.to_string()]
        });
        let out = normalize_work_item_body(body, &states, &labels).unwrap();
        assert_eq!(out["state"], json!(sid.to_string()));
        let arr = out["labels"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], json!(lid.to_string()));
        assert_eq!(arr[1], json!(lid.to_string()));
    }

    #[test]
    fn config_debug_never_prints_the_key() {
        let c = crate::Config {
            api_base: url::Url::parse("https://plane.example").unwrap(),
            api_key: "plane_api_debug_leak_check".into(),
            workspace_slug: "ws".into(),
            cache_enabled: false,
            json_output: true,
            acting_as: Some("bob".into()),
        };
        let s = format!("{c:?}");
        assert!(!s.contains("plane_api_debug_leak_check"), "{s}");
        assert!(s.contains("<redacted>") && s.contains("bob"), "{s}");
    }
}
