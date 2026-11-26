use plane_core::{print_json, take_warnings, CoreError, ResponseEnvelope};
use serde_json::Value;
use std::io::{self, Write};

fn write_stdout_line(text: &str) -> Result<(), CoreError> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())?;
    lock.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn write_stderr_line(text: &str) -> Result<(), CoreError> {
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(text.as_bytes())?;
    lock.write_all(b"\n")?;
    Ok(())
}

/// Human mode: print warnings queued below dispatch (JSON mode merges them in `print_json`).
fn write_pending_warnings_human() -> Result<(), CoreError> {
    for w in take_warnings() {
        write_stderr_line(&format!("warning: {w}"))?;
    }
    Ok(())
}

pub fn emit_success(
    json_mode: bool,
    data: Value,
    next_cursor: Option<String>,
    truncated: bool,
) -> Result<(), CoreError> {
    emit_success_with_warnings(json_mode, data, next_cursor, truncated, vec![])
}

pub fn emit_success_with_warnings(
    json_mode: bool,
    data: Value,
    next_cursor: Option<String>,
    truncated: bool,
    warnings: Vec<String>,
) -> Result<(), CoreError> {
    let truncated_opt = truncated.then_some(true);
    if json_mode {
        let env = ResponseEnvelope::success_meta(data, next_cursor, truncated_opt)
            .with_warnings(warnings);
        print_json(&env)?;
    } else {
        write_pending_warnings_human()?;
        for w in &warnings {
            write_stderr_line(&format!("warning: {w}"))?;
        }
        write_stdout_line(&serde_json::to_string_pretty(&data).map_err(CoreError::Json)?)?;
        if let Some(ref c) = next_cursor {
            write_stderr_line(&format!("next_cursor: {c}"))?;
        }
        if truncated {
            write_stderr_line("warning: results truncated (safety cap)")?;
        }
    }
    Ok(())
}

pub fn emit_empty_success(json_mode: bool) -> Result<(), CoreError> {
    if json_mode {
        print_json(&ResponseEnvelope::success(Value::Null))?;
    } else {
        write_pending_warnings_human()?;
        write_stderr_line("ok")?;
    }
    Ok(())
}

/// Error envelope as JSON with any API key (sent key, or a `plane_api_…` token) redacted from
/// the message, details and warnings.
pub fn error_envelope_value(err: &CoreError) -> Result<Value, CoreError> {
    let mut env = ResponseEnvelope::from_core_error(err);
    env.warnings = take_warnings();
    let mut v = serde_json::to_value(&env)?;
    crate::redact::redact_value(&mut v);
    Ok(v)
}

pub fn emit_err(json_mode: bool, err: &CoreError) -> Result<(), CoreError> {
    if json_mode {
        let v = error_envelope_value(err)?;
        write_stdout_line(&serde_json::to_string(&v)?)?;
    } else {
        for w in take_warnings() {
            write_stderr_line(&format!("warning: {}", crate::redact::redact_str(&w)))?;
        }
        write_stderr_line(&crate::redact::redact_str(&err.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_redacts_the_sent_key_from_message_and_details() {
        let sent = "custom-shaped-sent-key-77";
        crate::redact::register_secret(sent);
        let err = CoreError::Http {
            status: 400,
            method: "POST".into(),
            path: "/api/v1/x/".into(),
            message: format!("bad header {sent}"),
            body: Some(serde_json::json!({
                "detail": format!("bad header {sent}"),
                "echo": ["plane_api_pasted_into_body"],
            })),
            rate_limit: None,
        };
        plane_core::push_warning(format!("warning mentions {sent}"));
        let v = error_envelope_value(&err).unwrap();
        let s = v.to_string();
        assert!(!s.contains(sent), "{s}");
        assert!(!s.contains("plane_api_pasted"), "{s}");
        assert!(s.contains("<redacted>"), "{s}");
        assert_eq!(v["ok"], serde_json::json!(false));
        assert_eq!(v["error"]["code"], serde_json::json!("validation_error"));
    }
}
