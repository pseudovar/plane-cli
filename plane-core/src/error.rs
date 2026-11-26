use crate::envelope::RateLimitInfo;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Ok,
    Usage,
    Failure,
}

impl ExitKind {
    pub fn code(self) -> i32 {
        match self {
            ExitKind::Ok => 0,
            ExitKind::Usage => 2,
            ExitKind::Failure => 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("configuration: {0}")]
    Config(String),

    #[error("invalid usage: {0}")]
    Usage(String),

    /// Usage error with its own stable `error.code` (exit 2), e.g. `body_file_not_found`.
    #[error("invalid usage: {message}")]
    UsageCode { code: &'static str, message: String },

    #[error("HTTP {status} {method} {path}: {message}")]
    Http {
        status: u16,
        method: String,
        path: String,
        message: String,
        body: Option<Value>,
        rate_limit: Option<RateLimitInfo>,
    },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Network failure with the API host and the config layer that supplied
    /// `PLANE_API_BASE_URL` (built by the CLI from [`CoreError::Network`]; no secrets).
    #[error("network error: {message}")]
    Unreachable {
        message: String,
        api_host: String,
        config_layer: String,
        config_path: Option<String>,
        reason: String,
    },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    #[error("resolve: {0}")]
    Resolve(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cache: {0}")]
    Cache(String),

    /// Plane answered 2xx but the body lacks something the command needs.
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),

    /// Another error plus context: `message` replaces its text, `details` goes to
    /// `error.details`; the exit code and `error.code` stay those of `inner`.
    #[error("{message}")]
    WithDetails {
        message: String,
        inner: Box<CoreError>,
        details: Value,
    },
}

impl CoreError {
    pub fn exit_kind(&self) -> ExitKind {
        match self {
            CoreError::Usage(_)
            | CoreError::UsageCode { .. }
            | CoreError::Config(_)
            | CoreError::Resolve(_) => ExitKind::Usage,
            CoreError::Http { .. }
            | CoreError::Network(_)
            | CoreError::Unreachable { .. }
            | CoreError::Json(_)
            | CoreError::Url(_)
            | CoreError::Io(_)
            | CoreError::Cache(_)
            | CoreError::UnexpectedResponse(_) => ExitKind::Failure,
            CoreError::WithDetails { inner, .. } => inner.exit_kind(),
        }
    }

    /// Maps HTTP status to stable error codes for JSON output.
    pub fn error_code(&self) -> &'static str {
        match self {
            CoreError::Usage(_) | CoreError::Config(_) | CoreError::Resolve(_) => {
                "validation_error"
            }
            CoreError::UsageCode { code, .. } => code,
            CoreError::Http { status, .. } => match *status {
                400 => "validation_error",
                401 => "authentication_failed",
                403 => "permission_denied",
                404 => "not_found",
                409 => "conflict",
                429 => "rate_limited",
                500..=599 => "server_error",
                _ => "http_error",
            },
            CoreError::Network(_) | CoreError::Unreachable { .. } => "network_error",
            CoreError::Json(_) => "json_error",
            CoreError::Url(_) => "url_error",
            CoreError::Io(_) => "io_error",
            CoreError::Cache(_) => "cache_error",
            CoreError::UnexpectedResponse(_) => "unexpected_response",
            CoreError::WithDetails { inner, .. } => inner.error_code(),
        }
    }

    /// Wrap with context, keeping this error's kind and exit code. `note` is appended to
    /// the message in parentheses.
    pub fn with_details(self, note: &str, details: Value) -> CoreError {
        let message = if note.is_empty() {
            self.to_string()
        } else {
            format!("{self} ({note})")
        };
        CoreError::WithDetails {
            message,
            inner: Box::new(self),
            details,
        }
    }

    /// The innermost error (through any `WithDetails` layers).
    pub fn root(&self) -> &CoreError {
        match self {
            CoreError::WithDetails { inner, .. } => inner.root(),
            other => other,
        }
    }

    /// Shorthand for [`CoreError::UsageCode`].
    pub fn usage_code(code: &'static str, message: impl Into<String>) -> Self {
        CoreError::UsageCode {
            code,
            message: message.into(),
        }
    }

    pub fn is_broken_pipe(&self) -> bool {
        matches!(self, CoreError::Io(err) if err.kind() == std::io::ErrorKind::BrokenPipe)
    }
}
