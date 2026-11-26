use crate::error::CoreError;
use std::env;
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub api_base: Url,
    pub api_key: String,
    pub workspace_slug: String,
    pub cache_enabled: bool,
    pub json_output: bool,
    /// Plane member this call acts as (`--as`); `None` for the default `PLANE_API_KEY`.
    pub acting_as: Option<String>,
}

/// Never prints the API key, so a stray `{:?}` cannot leak it.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("api_base", &self.api_base.as_str())
            .field("api_key", &"<redacted>")
            .field("workspace_slug", &self.workspace_slug)
            .field("cache_enabled", &self.cache_enabled)
            .field("json_output", &self.json_output)
            .field("acting_as", &self.acting_as)
            .finish()
    }
}

fn env_truthy(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let v = v.trim().to_lowercase();
            v == "1" || v == "true" || v == "yes"
        }
        Err(_) => false,
    }
}

/// Machine-readable JSON mode: `--json` or `PLANE_CLI_JSON` env (see `docs/src/agent-contract.md`).
/// Must match [`Config::from_env`] `json_output` logic.
pub fn effective_json_output(cli_json_flag: bool) -> bool {
    cli_json_flag || env_truthy("PLANE_CLI_JSON")
}

fn normalize_base(s: String) -> Result<Url, CoreError> {
    let t = s.trim().trim_end_matches('/').to_string();
    Url::parse(&t).map_err(|e| CoreError::Config(format!("PLANE_API_BASE_URL: {e}")))
}

/// A workspace slug is one URL path segment: letters, digits, `-` and `_`. Anything else could
/// change which API path a request reaches.
fn validate_slug(slug: &str) -> Result<(), CoreError> {
    if slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(CoreError::Config(
            "PLANE_WORKSPACE_SLUG may contain only letters, digits, `-` and `_`".into(),
        ))
    }
}

impl Config {
    /// Reads `PLANE_API_BASE_URL`, `PLANE_API_KEY` and `PLANE_WORKSPACE_SLUG`.
    pub fn from_env(cli_json: bool) -> Result<Self, CoreError> {
        let base_raw = env::var("PLANE_API_BASE_URL")
            .map_err(|_| CoreError::Config("PLANE_API_BASE_URL is not set".into()))?;
        normalize_base(base_raw)?;
        let api_key = env::var("PLANE_API_KEY")
            .map_err(|_| CoreError::Config("PLANE_API_KEY is not set".into()))?;
        if api_key.is_empty() {
            return Err(CoreError::Config("PLANE_API_KEY is empty".into()));
        }
        Self::from_env_with_key(cli_json, api_key, None)
    }

    /// Like [`Config::from_env`], but with an API key loaded elsewhere (a member key for
    /// `--as`); `PLANE_API_KEY` is not read.
    pub fn from_env_with_key(
        cli_json: bool,
        api_key: String,
        acting_as: Option<String>,
    ) -> Result<Self, CoreError> {
        if api_key.trim().is_empty() {
            return Err(CoreError::Config("API key is empty".into()));
        }
        let base_raw = env::var("PLANE_API_BASE_URL")
            .map_err(|_| CoreError::Config("PLANE_API_BASE_URL is not set".into()))?;
        let api_base = normalize_base(base_raw)?;
        let workspace_slug = env::var("PLANE_WORKSPACE_SLUG")
            .map_err(|_| CoreError::Config("PLANE_WORKSPACE_SLUG is not set".into()))?;
        if workspace_slug.trim().is_empty() {
            return Err(CoreError::Config("PLANE_WORKSPACE_SLUG is empty".into()));
        }
        validate_slug(workspace_slug.trim())?;

        let cache_enabled = !env::var("PLANE_CACHE")
            .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
            .unwrap_or(false);

        let json_output = effective_json_output(cli_json);

        Ok(Self {
            api_base,
            api_key,
            workspace_slug: workspace_slug.trim().to_string(),
            cache_enabled,
            json_output,
            acting_as,
        })
    }

    pub fn with_json_override(&self, cli_json: bool) -> Config {
        let mut c = self.clone();
        if cli_json {
            c.json_output = true;
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::validate_slug;

    #[test]
    fn slug_cannot_change_the_request_path() {
        for ok in ["acme", "acme-eng", "team_2", "ACME"] {
            assert!(validate_slug(ok).is_ok(), "{ok}");
        }
        for bad in [
            "..", "a/b", "a?b", "a#b", "a%2fb", "a b", "a\\b", "../users",
        ] {
            assert!(validate_slug(bad).is_err(), "{bad}");
        }
    }
}
