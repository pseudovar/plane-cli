use crate::error::CoreError;
use crate::warnings::push_warning;
use serde_json::{Map, Value};
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::Path;

/// Warning attached when `--body` carries inline JSON instead of a path.
pub const INLINE_BODY_WARNING: &str =
    "`--body` takes a file path or `-`; inline JSON accepted, pipe it on stdin instead (`--body -`)";

/// Load JSON from `-` (stdin) or a file path.
///
/// Inline JSON (a value starting with `{` or `[` that parses) is accepted with a queued
/// warning (see [`crate::take_warnings`]). A missing or unreadable file is a usage error
/// with `error.code` `body_file_not_found` / `body_file_unreadable`, not an I/O failure.
pub fn load_json_source(src: Option<&str>) -> Result<Option<Value>, CoreError> {
    let Some(s) = src else {
        return Ok(None);
    };
    let (value, warning) = load_json_source_from(s, &mut std::io::stdin())?;
    if let Some(w) = warning {
        push_warning(w);
    }
    Ok(Some(value))
}

/// Testable core of [`load_json_source`]: returns the value and an optional warning.
fn load_json_source_from(
    s: &str,
    stdin: &mut dyn Read,
) -> Result<(Value, Option<String>), CoreError> {
    if s == "-" {
        let mut buf = String::new();
        stdin.read_to_string(&mut buf)?;
        let v = serde_json::from_str(&buf).map_err(|e| {
            CoreError::usage_code(
                "body_invalid_json",
                format!("`--body -`: stdin is not valid JSON: {e}"),
            )
        })?;
        return Ok((v, None));
    }

    let trimmed = s.trim_start();
    let looks_inline = trimmed.starts_with('{') || trimmed.starts_with('[');
    if looks_inline {
        match serde_json::from_str::<Value>(s) {
            Ok(v) => return Ok((v, Some(INLINE_BODY_WARNING.to_string()))),
            Err(e) if !Path::new(s).exists() => {
                return Err(CoreError::usage_code(
                    "body_invalid_json",
                    format!(
                        "`--body` value looks like inline JSON but does not parse: {e}. \
                         `--body` takes a file path or `-` (pipe JSON on stdin)"
                    ),
                ));
            }
            // A file whose name starts with `{` or `[`: fall through and read it.
            Err(_) => {}
        }
    }

    let path = Path::new(s);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(CoreError::usage_code(
                "body_file_not_found",
                format!(
                    "`--body` file not found: {s}. `--body` takes a file path or `-` (pipe JSON on stdin)"
                ),
            ));
        }
        Err(e) => {
            return Err(CoreError::usage_code(
                "body_file_unreadable",
                format!(
                    "`--body` file could not be read: {s} ({e}). `--body` takes a file path or `-` (pipe JSON on stdin)"
                ),
            ));
        }
    };
    let v = serde_json::from_str(&raw).map_err(|e| {
        CoreError::usage_code(
            "body_invalid_json",
            format!("`--body` file {s} is not valid JSON: {e}"),
        )
    })?;
    Ok((v, None))
}

/// Merge `patch` into `base` for JSON objects (shallow keys from patch win).
pub fn merge_objects(base: Value, patch: Value) -> Value {
    let mut base_map = match base {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if let Value::Object(p) = patch {
        for (k, v) in p {
            base_map.insert(k, v);
        }
    }
    Value::Object(base_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn load(s: &str) -> Result<(Value, Option<String>), CoreError> {
        load_json_source_from(s, &mut std::io::empty())
    }

    fn code(err: CoreError) -> &'static str {
        assert_eq!(err.exit_kind().code(), 2, "body errors exit 2: {err}");
        err.error_code()
    }

    #[test]
    fn inline_object_is_accepted_with_warning() {
        let (v, w) = load(r#"{"name":"x"}"#).unwrap();
        assert_eq!(v, json!({"name": "x"}));
        assert_eq!(w.as_deref(), Some(INLINE_BODY_WARNING));
    }

    #[test]
    fn inline_array_is_accepted_with_warning() {
        let (v, w) = load(r#"  [1, 2]"#).unwrap();
        assert_eq!(v, json!([1, 2]));
        assert!(w.is_some());
    }

    #[test]
    fn invalid_inline_json_is_usage_error() {
        let err = load(r#"{"name":"#).unwrap_err();
        assert_eq!(code(err), "body_invalid_json");
    }

    #[test]
    fn missing_file_names_path() {
        let err = load("/nope/plane-cli-missing.json").unwrap_err();
        assert!(err.to_string().contains("/nope/plane-cli-missing.json"));
        assert_eq!(code(err), "body_file_not_found");
    }

    #[test]
    fn directory_is_unreadable() {
        let dir = std::env::temp_dir();
        let err = load(dir.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains(dir.to_str().unwrap()));
        assert_eq!(code(err), "body_file_unreadable");
    }

    #[test]
    fn file_with_invalid_json_is_usage_error() {
        let path = std::env::temp_dir().join(format!("plane-cli-bad-{}.json", std::process::id()));
        fs::write(&path, "not json").unwrap();
        let err = load(path.to_str().unwrap()).unwrap_err();
        let _ = fs::remove_file(&path);
        assert_eq!(code(err), "body_invalid_json");
    }

    #[test]
    fn file_is_read_without_warning() {
        let path = std::env::temp_dir().join(format!("plane-cli-ok-{}.json", std::process::id()));
        fs::write(&path, r#"{"a":1}"#).unwrap();
        let out = load(path.to_str().unwrap());
        let _ = fs::remove_file(&path);
        let (v, w) = out.unwrap();
        assert_eq!(v, json!({"a": 1}));
        assert!(w.is_none());
    }

    #[test]
    fn stdin_is_read_without_warning() {
        let mut input: &[u8] = br#"{"comment_html":"<p>hi</p>"}"#;
        let (v, w) = load_json_source_from("-", &mut input).unwrap();
        assert_eq!(v["comment_html"], "<p>hi</p>");
        assert!(w.is_none());
    }

    #[test]
    fn invalid_stdin_is_usage_error() {
        let mut input: &[u8] = b"nope";
        let err = load_json_source_from("-", &mut input).unwrap_err();
        assert_eq!(code(err), "body_invalid_json");
    }
}
