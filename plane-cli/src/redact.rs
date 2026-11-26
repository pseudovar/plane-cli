//! Keep API keys out of error output and telemetry.
//!
//! Two layers: every key the CLI sends is registered with [`register_secret`] and replaced by
//! `<redacted>` wherever it appears, and any token that looks like a Plane key (`plane_api_…`)
//! is replaced too, which catches keys pasted into argv by mistake.

use serde_json::Value;
use std::sync::{Mutex, OnceLock};

pub const REDACTED: &str = "<redacted>";
const KEY_PREFIX: &str = "plane_api_";
/// Shorter values are not registered: replacing them could mangle ordinary text.
const MIN_SECRET_LEN: usize = 8;

fn secrets() -> &'static Mutex<Vec<String>> {
    static S: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

/// Remember a key that this process sends, so it is redacted from errors and telemetry.
pub fn register_secret(secret: &str) {
    let s = secret.trim();
    if s.len() < MIN_SECRET_LEN {
        return;
    }
    let mut v = secrets().lock().unwrap_or_else(|e| e.into_inner());
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Register `PLANE_API_KEY` and every `PLANE_API_KEY_<MEMBER>` currently in the environment.
pub fn register_env_keys() {
    for (k, v) in std::env::vars() {
        if k == "PLANE_API_KEY" || k.starts_with("PLANE_API_KEY_") {
            register_secret(&v);
        }
    }
}

/// Replace registered keys and `plane_api_…` tokens with `<redacted>`.
pub fn redact_str(input: &str) -> String {
    let mut out = input.to_string();
    {
        let v = secrets().lock().unwrap_or_else(|e| e.into_inner());
        for s in v.iter() {
            if out.contains(s.as_str()) {
                out = out.replace(s.as_str(), REDACTED);
            }
        }
    }
    redact_key_tokens(&out)
}

/// Replace each `plane_api_` token (through the following run of `[A-Za-z0-9_-]`).
fn redact_key_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(i) = rest.find(KEY_PREFIX) {
        out.push_str(&rest[..i]);
        let tail = &rest[i + KEY_PREFIX.len()..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(tail.len());
        out.push_str(REDACTED);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// [`redact_str`] on every string in a JSON value (keys included).
pub fn redact_value(v: &mut Value) {
    match v {
        Value::String(s) => {
            let r = redact_str(s);
            if r != *s {
                *s = r;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact_value),
        Value::Object(m) => {
            let entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
            for (k, mut val) in entries {
                redact_value(&mut val);
                m.insert(redact_str(&k), val);
            }
        }
        _ => {}
    }
}

/// The value clap names as the offending argument, when it is not a flag (a stray
/// positional could be anything, including a pasted key).
pub fn clap_positional_value(err: &clap::Error) -> Option<String> {
    use clap::error::ContextKind;
    err.get(ContextKind::InvalidArg)
        .map(|v| v.to_string())
        .filter(|a| !a.starts_with('-'))
}

/// Clap's rendered message with a stray positional replaced by `<positional>`, then
/// [`redact_str`].
/// `positional` comes from [`clap_positional_value`] (taken before `format` consumes the error).
pub fn redact_clap_text(positional: Option<&str>, rendered: &str) -> String {
    let mut s = rendered.to_string();
    if let Some(v) = positional.filter(|v| !v.is_empty()) {
        s = s.replace(&format!("'{v}'"), "'<positional>'");
        s = s.replace(v, "<positional>");
    }
    redact_str(&s)
}

/// Scrub a Sentry event (message, exception values, extra, tags) in place.
pub fn redact_event(ev: &mut sentry::protocol::Event<'static>) {
    if let Some(m) = ev.message.as_mut() {
        *m = redact_str(m);
    }
    for ex in ev.exception.values.iter_mut() {
        if let Some(v) = ex.value.as_mut() {
            *v = redact_str(v);
        }
    }
    if let Some(le) = ev.logentry.as_mut() {
        le.message = redact_str(&le.message);
    }
    for v in ev.extra.values_mut() {
        redact_value(v);
    }
    for v in ev.tags.values_mut() {
        *v = redact_str(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_shaped_tokens_are_redacted() {
        assert_eq!(
            redact_str("bad token plane_api_ABCdef123_x-y found"),
            "bad token <redacted> found"
        );
        assert_eq!(redact_str("'plane_api_FAKE'"), "'<redacted>'");
        assert_eq!(redact_str("nothing here"), "nothing here");
    }

    #[test]
    fn registered_secrets_are_redacted_anywhere() {
        register_secret("sk-unusual-shape-0042");
        register_secret("short"); // ignored
        assert_eq!(
            redact_str("echo sk-unusual-shape-0042!"),
            "echo <redacted>!"
        );
        assert_eq!(redact_str("short"), "short");
        let mut v = json!({"detail": ["x sk-unusual-shape-0042"], "sk-unusual-shape-0042": 1});
        redact_value(&mut v);
        let s = v.to_string();
        assert!(!s.contains("sk-unusual-shape-0042"), "{s}");
    }

    #[test]
    fn clap_stray_positional_is_replaced() {
        use crate::dispatch::Cli;
        use clap::{CommandFactory, Parser};
        let err = Cli::try_parse_from([
            "plane-cli",
            "--json",
            "config",
            "set-key",
            "--member",
            "bob",
            "notakeybutsecret",
        ])
        .err()
        .unwrap();
        let pos = clap_positional_value(&err);
        assert_eq!(pos.as_deref(), Some("notakeybutsecret"));
        let rendered = err.format(&mut Cli::command()).to_string();
        assert!(rendered.contains("notakeybutsecret"));
        let red = redact_clap_text(pos.as_deref(), &rendered);
        assert!(!red.contains("notakeybutsecret"), "{red}");
        assert!(red.contains("<positional>"), "{red}");
    }
}
