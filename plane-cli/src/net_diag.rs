//! Network diagnostics: say which host failed and which config layer set
//! `PLANE_API_BASE_URL`, and probe reachability for `config doctor`. No secret values.

use crate::plane_env_files::{base_url_source, ValueSource};
use plane_core::{Config, CoreError, HttpClient};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// Timeout for the `config doctor` reachability probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Innermost cause of an error chain, e.g. `Connection refused (os error 111)`.
pub fn root_cause(err: &(dyn std::error::Error + 'static)) -> String {
    let mut cur = err;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

/// `host[:port]` of a base URL string, without scheme, path or credentials.
pub fn host_of(base: &str) -> Option<String> {
    let t = base.trim().trim_end_matches('/');
    let rest = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let authority = rest.split('/').next()?;
    let host = authority.rsplit('@').next()?;
    (!host.is_empty()).then(|| host.to_string())
}

/// Host of the configured base URL, or `<unset>`.
pub fn configured_host() -> String {
    std::env::var("PLANE_API_BASE_URL")
        .ok()
        .and_then(|b| host_of(&b))
        .unwrap_or_else(|| "<unset>".into())
}

/// Turn a bare [`CoreError::Network`] into [`CoreError::Unreachable`], naming the host that was
/// tried and the config layer that supplied `PLANE_API_BASE_URL`. Other errors pass through.
pub fn enrich_network_error(err: CoreError, env_file: Option<&Path>, dotenv: bool) -> CoreError {
    let CoreError::Network(e) = &err else {
        return err;
    };
    let api_host = e
        .url()
        .and_then(|u| {
            let host = u.host_str()?;
            Some(match u.port() {
                Some(p) => format!("{host}:{p}"),
                None => host.to_string(),
            })
        })
        .unwrap_or_else(configured_host);
    let reason = root_cause(e);
    let source = base_url_source(env_file, dotenv);
    unreachable_error(api_host, &source, reason)
}

fn unreachable_error(api_host: String, source: &ValueSource, reason: String) -> CoreError {
    let message = format!(
        "cannot reach Plane API at {api_host} (PLANE_API_BASE_URL from {}): {reason}",
        source.describe()
    );
    CoreError::Unreachable {
        message,
        api_host,
        config_layer: source.layer.to_string(),
        config_path: source.path.as_ref().map(|p| p.display().to_string()),
        reason,
    }
}

/// Outcome of the `config doctor` reachability check.
#[derive(Debug, PartialEq, Eq)]
pub enum Reachability {
    Reachable,
    Unreachable { reason: String },
    Unauthorized { http_status: u16 },
    NotPlane { http_status: u16 },
    Skipped { reason: String },
}

impl Reachability {
    pub fn status(&self) -> &'static str {
        match self {
            Reachability::Reachable => "reachable",
            Reachability::Unreachable { .. } => "unreachable",
            Reachability::Unauthorized { .. } => "unauthorized",
            Reachability::NotPlane { .. } => "not_plane",
            Reachability::Skipped { .. } => "skipped",
        }
    }

    pub fn to_json(&self, api_host: &str, source: &ValueSource) -> Value {
        let (reason, http_status) = match self {
            Reachability::Unreachable { reason } | Reachability::Skipped { reason } => {
                (Some(reason.clone()), None)
            }
            Reachability::Unauthorized { http_status } | Reachability::NotPlane { http_status } => {
                (None, Some(*http_status))
            }
            Reachability::Reachable => (None, None),
        };
        json!({
            "status": self.status(),
            "api_host": api_host,
            "base_url_source": source.to_json(),
            "reason": reason,
            "http_status": http_status,
        })
    }
}

/// Classify a `GET /api/v1/users/me/` response: Plane answers 2xx with the user object.
pub fn classify_users_me(status: u16, body: Option<&Value>) -> Reachability {
    match status {
        200..=299 => {
            let is_user = body.and_then(Value::as_object).is_some_and(|o| {
                o.contains_key("id") && (o.contains_key("email") || o.contains_key("display_name"))
            });
            if is_user {
                Reachability::Reachable
            } else {
                Reachability::NotPlane {
                    http_status: status,
                }
            }
        }
        401 | 403 => Reachability::Unauthorized {
            http_status: status,
        },
        _ => Reachability::NotPlane {
            http_status: status,
        },
    }
}

/// One `users/me` call with [`PROBE_TIMEOUT`].
pub fn probe(config: &Config) -> Reachability {
    let client = match HttpClient::with_timeout(config, PROBE_TIMEOUT) {
        Ok(c) => c,
        Err(e) => {
            return Reachability::Unreachable {
                reason: e.to_string(),
            }
        }
    };
    match client.get("/api/v1/users/me/", &[]) {
        Ok((status, body, _)) => classify_users_me(status, body.as_ref()),
        Err(CoreError::Network(e)) => Reachability::Unreachable {
            reason: root_cause(&e),
        },
        Err(e) => Reachability::Unreachable {
            reason: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_keeps_port_and_drops_path_and_userinfo() {
        assert_eq!(
            host_of("http://127.0.0.1:9").as_deref(),
            Some("127.0.0.1:9")
        );
        assert_eq!(
            host_of("https://user:pw@plane.example.com/api/").as_deref(),
            Some("plane.example.com")
        );
        assert_eq!(host_of(""), None);
    }

    #[test]
    fn users_me_classification() {
        let user = json!({"id": "u1", "email": "a@b.c", "display_name": "a"});
        assert_eq!(classify_users_me(200, Some(&user)), Reachability::Reachable);
        assert_eq!(
            classify_users_me(401, None),
            Reachability::Unauthorized { http_status: 401 }
        );
        assert_eq!(
            classify_users_me(200, Some(&json!("<html>"))),
            Reachability::NotPlane { http_status: 200 }
        );
        assert_eq!(
            classify_users_me(404, None),
            Reachability::NotPlane { http_status: 404 }
        );
    }

    #[test]
    fn unreachable_message_names_host_and_layer() {
        let src = ValueSource {
            layer: "global",
            path: Some("/home/x/.config/plane-cli/.plane-cli.env".into()),
        };
        let err = unreachable_error(
            "127.0.0.1:9".into(),
            &src,
            "Connection refused (os error 111)".into(),
        );
        assert_eq!(err.error_code(), "network_error");
        let msg = err.to_string();
        assert!(msg.contains("127.0.0.1:9"));
        assert!(msg.contains("global file /home/x/.config/plane-cli/.plane-cli.env"));
    }
}
