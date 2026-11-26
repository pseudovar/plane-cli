use crate::error::CoreError;
use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write};

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitInfo {
    pub remaining: Option<u64>,
    pub reset_epoch: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct HttpErrorDetail {
    pub status: u16,
    pub method: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpErrorDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct ResponseEnvelope {
    pub ok: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorEnvelope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

impl ResponseEnvelope {
    pub fn success(data: Value) -> Self {
        Self {
            ok: true,
            warnings: vec![],
            data: Some(data),
            error: None,
            next_cursor: None,
            truncated: None,
        }
    }

    pub fn success_meta(data: Value, next_cursor: Option<String>, truncated: Option<bool>) -> Self {
        Self {
            ok: true,
            warnings: vec![],
            data: Some(data),
            error: None,
            next_cursor,
            truncated,
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn from_core_error(err: &CoreError) -> Self {
        let (http, rate_limit, details) = match err {
            CoreError::WithDetails { inner, details, .. } => {
                let base = Self::from_core_error(inner).error;
                let (http, rate_limit, inner_details) = match base {
                    Some(e) => (e.http, e.rate_limit, e.details),
                    None => (None, None, None),
                };
                let mut merged = details.clone();
                if let (Some(obj), Some(d)) = (merged.as_object_mut(), inner_details) {
                    obj.entry("cause").or_insert(d);
                }
                (http, rate_limit, Some(merged))
            }
            CoreError::Http {
                status,
                method,
                path,
                message: _,
                body,
                rate_limit,
            } => (
                Some(HttpErrorDetail {
                    status: *status,
                    method: method.clone(),
                    path: path.clone(),
                }),
                rate_limit.clone(),
                body.clone(),
            ),
            _ => (None, None, derived_error_details(err)),
        };

        Self {
            ok: false,
            warnings: vec![],
            data: None,
            error: Some(ErrorEnvelope {
                code: err.error_code().to_string(),
                message: err.to_string(),
                http,
                rate_limit,
                details,
            }),
            next_cursor: None,
            truncated: None,
        }
    }
}

fn derived_error_details(err: &CoreError) -> Option<Value> {
    match err {
        CoreError::Usage(msg) | CoreError::Resolve(msg) | CoreError::Config(msg) => {
            usage_details_for_message(msg)
        }
        CoreError::UsageCode { code, message } => usage_code_details(code, message),
        CoreError::Unreachable {
            api_host,
            config_layer,
            config_path,
            reason,
            ..
        } => Some(serde_json::json!({
            "api_host": api_host,
            "config_layer": config_layer,
            "config_path": config_path,
            "reason": reason,
            "hint": "PLANE_API_BASE_URL may be stale or the host down. Run `plane-cli --json config where` to see which file sets it and `plane-cli --json config doctor` to test reachability.",
            "suggested_command": "plane-cli --json config doctor"
        })),
        _ => None,
    }
}

fn usage_code_details(code: &str, message: &str) -> Option<Value> {
    match code {
        "body_file_not_found" | "body_file_unreadable" | "body_invalid_json" => {
            Some(serde_json::json!({
                "hint": "`--body` takes a JSON file path or `-` to read JSON from stdin.",
                "suggested_command": "echo '{\"name\":\"...\"}' | plane-cli --json <COMMAND> --body -"
            }))
        }
        "project_mismatch" => Some(serde_json::json!({
            "hint": "A PREFIX-N work item already names its project: make --project match the prefix, or drop --project on commands where it is optional."
        })),
        "project_required_for_uuid" => Some(serde_json::json!({
            "hint": "Bare UUIDs need --project; composite ids like PROJ-123 do not.",
            "suggested_command": "plane-cli --json work-items get --work-item <PROJ-123>"
        })),
        _ => usage_details_for_message(message),
    }
}

fn usage_details_for_message(message: &str) -> Option<Value> {
    // `plane-cli states --project X` / `plane-cli labels ENG`: the area without a subcommand.
    for area in ["states", "labels"] {
        if message.contains(&format!("Usage: plane-cli {area} [OPTIONS] <COMMAND>")) {
            return Some(serde_json::json!({
                "hint": format!("`{area}` needs a subcommand: `{area} list --project <PROJECT>` (also create, update, delete)."),
                "suggested_command": format!("plane-cli --json {area} list --project <PROJECT>")
            }));
        }
    }
    if message.contains("unrecognized subcommand 'states'") {
        return Some(serde_json::json!({
            "hint": "States are a top-level command: `states list --project <PROJECT>` (or `complete states --project <PROJECT>` for names only).",
            "suggested_command": "plane-cli --json states list --project <PROJECT>"
        }));
    }
    if message.contains("unrecognized subcommand 'labels'") {
        return Some(serde_json::json!({
            "hint": "Labels are a top-level command: `labels list --project <PROJECT>` (or `complete labels --project <PROJECT>` for names only).",
            "suggested_command": "plane-cli --json labels list --project <PROJECT>"
        }));
    }
    if message.contains("unrecognized subcommand 'comment'") {
        return Some(serde_json::json!({
            "hint": "Comments live under `work-items comments create`, not `work-items comment`.",
            "suggested_command": "plane-cli --json work-items comments create --project <PROJECT> --work-item <ITEM> --body -"
        }));
    }
    if message.contains("unexpected argument '--labels' found") {
        return Some(serde_json::json!({
            "hint": "Use `--body PATH|-` for labels, e.g. `echo '{\"labels\":[\"bug\"]}' | plane-cli --json --strict-catalog work-items update --project <PROJECT> --work-item <ITEM> --body -`.",
            "suggested_command": "plane-cli --json --strict-catalog work-items update --project <PROJECT> --work-item <ITEM> --body -"
        }));
    }
    if message.contains("unexpected argument '--data' found") {
        return Some(serde_json::json!({
            "hint": "The CLI uses `--body PATH|-` for JSON payloads, not `--data`.",
            "suggested_command": "plane-cli --json <COMMAND> --body -"
        }));
    }
    if message.contains("Usage: plane-cli work-items get [OPTIONS] --work-item <ITEM>")
        && message.contains("unexpected argument")
    {
        return Some(serde_json::json!({
            "hint": "Pass the work item explicitly with `--work-item`. UUIDs also require `--project`.",
            "suggested_command": "plane-cli --json work-items get --work-item <ITEM>"
        }));
    }
    if message.contains("Usage: plane-cli work-items update [OPTIONS]")
        && message.contains("unexpected argument")
    {
        return Some(serde_json::json!({
            "hint": "Use explicit flags for update targets. Extra fields such as labels belong in `--body PATH|-`.",
            "suggested_command": "plane-cli --json work-items update --work-item <ITEM> [--description \"...\"] [--state \"...\"] [--body -]"
        }));
    }
    if message.contains("Next step: run `plane-cli config init`") {
        return Some(serde_json::json!({
            "hint": "Initialize a local or global plane-cli env file before invoking Plane API commands.",
            "suggested_command": "plane-cli config init"
        }));
    }
    if message.contains("unrecognized subcommand 'issue'") {
        return Some(serde_json::json!({
            "hint": "There is no `issue` command; use `work-items list --project <PROJECT> --json` to list work items.",
            "suggested_command": "plane-cli --json work-items list --project <PROJECT>"
        }));
    }
    if message.contains("unrecognized subcommand 'work-item'") {
        return Some(serde_json::json!({
            "hint": "The command is plural: `work-items` (not `work-item`). Example: `work-items list --project <PROJECT> --json`.",
            "suggested_command": "plane-cli --json work-items list --project <PROJECT>"
        }));
    }
    if message.contains("unrecognized subcommand 'ticket'") {
        return Some(serde_json::json!({
            "hint": "There is no `ticket` command; use `work-items list --project <PROJECT> --json`.",
            "suggested_command": "plane-cli --json work-items list --project <PROJECT>"
        }));
    }
    if message.contains("unrecognized subcommand 'project'") {
        return Some(serde_json::json!({
            "hint": "The command is plural: `projects` (not `project`). Example: `projects list --json`.",
            "suggested_command": "plane-cli --json projects list"
        }));
    }
    if message.contains("unrecognized subcommand 'state'")
        || message.contains("unknown command 'state'")
    {
        return Some(serde_json::json!({
            "hint": "The command is plural: `states list --project <PROJECT> --json`.",
            "suggested_command": "plane-cli --json states list --project <PROJECT>"
        }));
    }
    if message.contains("unknown command 'work-item'")
        || message.contains("unknown command 'issue'")
        || message.contains("unknown command 'ticket'")
        || message.contains("unknown command 'project'")
    {
        // Generic redirect for alias commands
        return Some(serde_json::json!({
            "hint": "See `plane-cli --help` for the correct command surface.",
            "suggested_command": "plane-cli --help"
        }));
    }
    None
}

fn write_json_to<W: Write>(env: &ResponseEnvelope, mut writer: W) -> Result<(), CoreError> {
    let body = serde_json::to_vec(env)?;
    writer.write_all(&body)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Print an envelope to stdout, prepending any queued warnings (see [`crate::push_warning`]).
pub fn print_json(env: &ResponseEnvelope) -> Result<(), CoreError> {
    let stdout = io::stdout();
    let lock = stdout.lock();
    let pending = crate::warnings::take_warnings();
    if pending.is_empty() {
        write_json_to(env, lock)?;
    } else {
        write_value_to(&with_pending_warnings(env, pending)?, lock)?;
    }
    Ok(())
}

fn with_pending_warnings(env: &ResponseEnvelope, pending: Vec<String>) -> Result<Value, CoreError> {
    let mut v = serde_json::to_value(env)?;
    if let Value::Object(map) = &mut v {
        let mut all: Vec<Value> = pending.into_iter().map(Value::String).collect();
        if let Some(Value::Array(existing)) = map.remove("warnings") {
            all.extend(existing);
        }
        map.insert("warnings".into(), Value::Array(all));
    }
    Ok(v)
}

fn write_value_to<W: Write>(v: &Value, mut writer: W) -> Result<(), CoreError> {
    let body = serde_json::to_vec(v)?;
    writer.write_all(&body)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn states_or_labels_without_subcommand_suggest_list() {
        for area in ["states", "labels"] {
            let env = ResponseEnvelope::from_core_error(&CoreError::Usage(format!(
                "error: unexpected argument '--project' found\n\nUsage: plane-cli {area} [OPTIONS] <COMMAND>\n"
            )));
            let details = env.error.unwrap().details.unwrap();
            assert_eq!(
                details.get("suggested_command").and_then(Value::as_str),
                Some(format!("plane-cli --json {area} list --project <PROJECT>").as_str())
            );
        }
    }

    #[test]
    fn usage_error_for_states_includes_agent_hint_details() {
        let env = ResponseEnvelope::from_core_error(&CoreError::Usage(
            "error: unrecognized subcommand 'states'".into(),
        ));

        let details = env.error.unwrap().details.unwrap();
        assert_eq!(
            details.get("suggested_command").and_then(Value::as_str),
            Some("plane-cli --json states list --project <PROJECT>")
        );
    }

    #[test]
    fn usage_error_for_labels_flag_includes_body_hint() {
        let env = ResponseEnvelope::from_core_error(&CoreError::Usage(
            "error: unexpected argument '--labels' found".into(),
        ));

        let details = env.error.unwrap().details.unwrap();
        assert_eq!(
            details.get("hint").and_then(Value::as_str),
            Some("Use `--body PATH|-` for labels, e.g. `echo '{\"labels\":[\"bug\"]}' | plane-cli --json --strict-catalog work-items update --project <PROJECT> --work-item <ITEM> --body -`.")
        );
    }

    #[test]
    fn pending_warnings_come_before_envelope_warnings() {
        let env = ResponseEnvelope::success(Value::Null).with_warnings(vec!["second".into()]);
        let v = with_pending_warnings(&env, vec!["first".into()]).unwrap();
        assert_eq!(v["ok"], Value::Bool(true));
        assert_eq!(v["warnings"], serde_json::json!(["first", "second"]));
    }

    #[test]
    fn usage_code_error_keeps_its_code_and_exit_2() {
        let err = CoreError::usage_code("body_file_not_found", "`--body` file not found: /x");
        let env = ResponseEnvelope::from_core_error(&err);
        let e = env.error.unwrap();
        assert_eq!(e.code, "body_file_not_found");
        assert!(e.details.unwrap().get("hint").is_some());
        assert_eq!(err.exit_kind().code(), 2);
    }

    #[test]
    fn write_json_to_returns_broken_pipe_io_error() {
        let env = ResponseEnvelope::success(Value::Null);
        let err = write_json_to(&env, BrokenPipeWriter).unwrap_err();
        match err {
            CoreError::Io(io) => assert_eq!(io.kind(), io::ErrorKind::BrokenPipe),
            other => panic!("expected broken pipe io error, got {other:?}"),
        }
    }
}
