use crate::config::Config;
use crate::envelope::RateLimitInfo;
use crate::error::CoreError;
use reqwest::blocking::RequestBuilder;
use reqwest::header::HeaderMap;
use serde_json::Value;
use url::Url;

/// Human-readable message from a Plane error body: `detail`, `message`, `error`, or the
/// first `non_field_errors` entry (CE 1.4.2 uses all four), else "request failed".
pub(crate) fn api_error_message(body: Option<&Value>) -> String {
    let Some(v) = body else {
        return "request failed".into();
    };
    for key in ["detail", "message", "error"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            return s.to_string();
        }
    }
    v.get("non_field_errors")
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .unwrap_or("request failed")
        .to_string()
}

/// Redirect limit for API requests, the same as reqwest's default.
const MAX_API_REDIRECTS: usize = 10;

/// Redirect policy for requests that carry the API key: reqwest would forward `X-API-Key` to
/// any host, so only same-origin (scheme, host, port) redirects are followed. Any other
/// redirect stops, and [`HttpClient::send_api`] reports it.
fn same_origin_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let same_origin = attempt
            .previous()
            .first()
            .is_some_and(|first| first.origin() == attempt.url().origin());
        if !same_origin {
            attempt.stop()
        } else if attempt.previous().len() > MAX_API_REDIRECTS {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

pub struct HttpClient {
    inner: reqwest::blocking::Client,
    /// Same TLS setup, but does not follow redirects (attachment download URLs).
    no_redirect: reqwest::blocking::Client,
    /// Follows redirects anywhere. Only for presigned storage URLs, and never sends the key.
    storage: reqwest::blocking::Client,
    base: Url,
    api_key: String,
}

impl HttpClient {
    pub fn new(config: &Config) -> Result<Self, CoreError> {
        let inner = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .redirect(same_origin_redirects())
            .build()?;
        let no_redirect = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let storage = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .build()?;
        Ok(Self {
            inner,
            no_redirect,
            storage,
            base: config.api_base.clone(),
            api_key: config.api_key.clone(),
        })
    }

    /// Resolve a URL returned by Plane (absolute, or relative to the API base).
    pub fn absolute_url(&self, url: &str) -> Result<Url, CoreError> {
        match Url::parse(url) {
            Ok(u) => Ok(u),
            Err(url::ParseError::RelativeUrlWithoutBase) => self.join_path(url),
            Err(e) => Err(e.into()),
        }
    }

    /// Authenticated GET that does not follow redirects. Returns the `Location` header of a
    /// 3xx response, or the error for anything else.
    pub fn get_redirect_location(&self, path: &str) -> Result<String, CoreError> {
        let url = self.join_path(path)?;
        let resp = self.apply_auth(self.no_redirect.get(url)).send()?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        if (300..400).contains(&status) {
            if let Some(loc) = headers.get("location").and_then(|v| v.to_str().ok()) {
                return Ok(loc.to_string());
            }
        }
        let rl = Self::parse_rate_limit(&headers);
        let text = resp.text()?;
        let body = if text.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        };
        let message = if (200..300).contains(&status) {
            "expected a redirect to the file".to_string()
        } else {
            api_error_message(body.as_ref())
        };
        Err(CoreError::Http {
            status,
            method: "GET".into(),
            path: path.into(),
            message,
            body,
            rate_limit: rl,
        })
    }

    /// Multipart POST of a file to a presigned storage URL (S3/MinIO `presigned_post`).
    /// The Plane API key is not sent: the signature in `fields` authorizes the upload.
    pub fn upload_presigned_post(
        &self,
        url: &str,
        fields: &serde_json::Map<String, Value>,
        file_name: &str,
        mime: &str,
        bytes: Vec<u8>,
    ) -> Result<(), CoreError> {
        let target = self.absolute_url(url)?;
        let mut form = reqwest::blocking::multipart::Form::new();
        for (k, v) in fields {
            let s = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            form = form.text(k.clone(), s);
        }
        let part = reqwest::blocking::multipart::Part::bytes(bytes)
            .file_name(file_name.to_string())
            .mime_str(mime)?;
        form = form.part("file", part);
        let resp = self.storage.post(target.clone()).multipart(form).send()?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let text = resp.text().unwrap_or_default();
        Err(CoreError::Http {
            status,
            method: "POST".into(),
            path: format!(
                "{}://{}{}",
                target.scheme(),
                target.host_str().unwrap_or(""),
                target.path()
            ),
            message: "file upload to storage failed".into(),
            body: (!text.is_empty()).then(|| Value::String(text.chars().take(2000).collect())),
            rate_limit: None,
        })
    }

    /// Unauthenticated GET of a presigned download URL, returning the bytes.
    pub fn download_presigned(&self, url: &str) -> Result<Vec<u8>, CoreError> {
        let target = self.absolute_url(url)?;
        let resp = self.storage.get(target.clone()).send()?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CoreError::Http {
                status,
                method: "GET".into(),
                path: format!(
                    "{}://{}{}",
                    target.scheme(),
                    target.host_str().unwrap_or(""),
                    target.path()
                ),
                message: "file download from storage failed".into(),
                body: None,
                rate_limit: None,
            });
        }
        Ok(resp.bytes()?.to_vec())
    }

    /// Client whose requests (connect included) give up after `timeout`; used for
    /// reachability probes such as `config doctor`.
    pub fn with_timeout(config: &Config, timeout: std::time::Duration) -> Result<Self, CoreError> {
        let inner = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .connect_timeout(timeout)
            .timeout(timeout)
            .redirect(same_origin_redirects())
            .build()?;
        let storage = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .connect_timeout(timeout)
            .timeout(timeout)
            .build()?;
        let no_redirect = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .connect_timeout(timeout)
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            inner,
            no_redirect,
            storage,
            base: config.api_base.clone(),
            api_key: config.api_key.clone(),
        })
    }

    /// Send a request carrying the API key. A redirect left unfollowed by
    /// [`same_origin_redirects`] is an error that names the other origin.
    fn send_api(
        &self,
        rb: RequestBuilder,
        method: &str,
        path: &str,
    ) -> Result<reqwest::blocking::Response, CoreError> {
        let resp = rb.send()?;
        let status = resp.status();
        if !status.is_redirection() || status.as_u16() == 304 {
            return Ok(resp);
        }
        let target = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .and_then(|l| resp.url().join(l).ok())
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|| "an unknown location".into());
        Err(CoreError::Http {
            status: status.as_u16(),
            method: method.into(),
            path: path.into(),
            message: format!(
                "Plane redirected to {target}, a different origin; not followed because it would expose the API key. Check PLANE_API_BASE_URL (scheme, host and port)"
            ),
            body: None,
            rate_limit: None,
        })
    }

    fn apply_auth(&self, rb: RequestBuilder) -> RequestBuilder {
        rb.header("X-API-Key", self.api_key.as_str())
    }

    fn join_path(&self, path: &str) -> Result<Url, CoreError> {
        let p = path.strip_prefix('/').unwrap_or(path);
        self.base
            .join(p)
            .map_err(|e| CoreError::Config(format!("bad URL join: {e}")))
    }

    fn parse_rate_limit(headers: &HeaderMap) -> Option<RateLimitInfo> {
        let remaining = headers
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let reset_epoch = headers
            .get("X-RateLimit-Reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        if remaining.is_none() && reset_epoch.is_none() {
            None
        } else {
            Some(RateLimitInfo {
                remaining,
                reset_epoch,
            })
        }
    }

    pub fn get(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<(u16, Option<Value>, Option<RateLimitInfo>), CoreError> {
        let url = self.join_path(path)?;
        let mut rb = self.inner.get(url);
        rb = self.apply_auth(rb);
        for (k, v) in query {
            rb = rb.query(&[(k.as_str(), v.as_str())]);
        }
        let resp = self.send_api(rb, "GET", path)?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let rl = Self::parse_rate_limit(&headers);
        let text = resp.text()?;
        let body = if text.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        };
        Ok((status, body, rl))
    }

    pub fn delete(
        &self,
        path: &str,
    ) -> Result<(u16, Option<Value>, Option<RateLimitInfo>), CoreError> {
        let url = self.join_path(path)?;
        let rb = self.apply_auth(self.inner.delete(url));
        let resp = self.send_api(rb, "DELETE", path)?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let rl = Self::parse_rate_limit(&headers);
        let text = resp.text()?;
        let body = if text.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        };
        Ok((status, body, rl))
    }

    pub fn post_json(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<(u16, Option<Value>, Option<RateLimitInfo>), CoreError> {
        let url = self.join_path(path)?;
        let rb = self.apply_auth(self.inner.post(url).json(body));
        let resp = self.send_api(rb, "POST", path)?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let rl = Self::parse_rate_limit(&headers);
        let text = resp.text()?;
        let parsed = if text.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        };
        Ok((status, parsed, rl))
    }

    pub fn patch_json(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<(u16, Option<Value>, Option<RateLimitInfo>), CoreError> {
        let url = self.join_path(path)?;
        let rb = self.apply_auth(self.inner.patch(url).json(body));
        let resp = self.send_api(rb, "PATCH", path)?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let rl = Self::parse_rate_limit(&headers);
        let text = resp.text()?;
        let parsed = if text.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&text).unwrap_or(Value::String(text)))
        };
        Ok((status, parsed, rl))
    }

    pub fn expect_json(
        &self,
        method: &str,
        path: &str,
        query: &[(String, String)],
        success: std::ops::RangeInclusive<u16>,
    ) -> Result<Value, CoreError> {
        let (status, body, rl) = self.get(path, query)?;
        Self::check_status(method, path, status, body, rl, success)
    }

    pub fn expect_json_post(
        &self,
        method: &str,
        path: &str,
        req_body: &Value,
        success: std::ops::RangeInclusive<u16>,
    ) -> Result<Value, CoreError> {
        let (status, body, rl) = self.post_json(path, req_body)?;
        Self::check_status(method, path, status, body, rl, success)
    }

    pub fn expect_json_patch(
        &self,
        method: &str,
        path: &str,
        req_body: &Value,
        success: std::ops::RangeInclusive<u16>,
    ) -> Result<Value, CoreError> {
        let (status, body, rl) = self.patch_json(path, req_body)?;
        Self::check_status(method, path, status, body, rl, success)
    }

    pub fn expect_delete(
        &self,
        path: &str,
        success: std::ops::RangeInclusive<u16>,
    ) -> Result<Option<Value>, CoreError> {
        let (status, body, rl) = self.delete(path)?;
        if success.contains(&status) {
            return Ok(body);
        }
        let message = api_error_message(body.as_ref());
        Err(CoreError::Http {
            status,
            method: "DELETE".into(),
            path: path.into(),
            message,
            body,
            rate_limit: rl,
        })
    }

    fn check_status(
        method: &str,
        path: &str,
        status: u16,
        body: Option<Value>,
        rate_limit: Option<RateLimitInfo>,
        success: std::ops::RangeInclusive<u16>,
    ) -> Result<Value, CoreError> {
        if success.contains(&status) {
            return Ok(body.unwrap_or(Value::Null));
        }
        let message = api_error_message(body.as_ref());
        Err(CoreError::Http {
            status,
            method: method.into(),
            path: path.into(),
            message,
            body,
            rate_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::api_error_message;
    use serde_json::json;

    #[test]
    fn error_message_reads_plane_error_shapes() {
        assert_eq!(api_error_message(Some(&json!({"detail": "d"}))), "d");
        assert_eq!(
            api_error_message(Some(
                &json!({"error": "The old cycle is not completed yet"})
            )),
            "The old cycle is not completed yet"
        );
        assert_eq!(
            api_error_message(Some(
                &json!({"non_field_errors": ["Cycles are not enabled for this project"]})
            )),
            "Cycles are not enabled for this project"
        );
        assert_eq!(
            api_error_message(Some(&json!({"name": ["x"]}))),
            "request failed"
        );
        assert_eq!(api_error_message(None), "request failed");
    }
}
