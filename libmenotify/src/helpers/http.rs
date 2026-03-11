use crate::MeNotifyError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

/// Blocking HTTP helper implementation for MeNotify.
pub struct MeNotifyHttp;

impl MeNotifyHttp {
    /// Validates and converts a request timeout into a `Duration`.
    ///
    /// Arguments:
    /// * `spec` - HTTP request specification.
    ///
    /// Returns:
    /// * `Ok(Duration)` when timeout is valid.
    /// * `Err(MeNotifyError)` when timeout is invalid.
    pub fn timeout(spec: &MeNotifyHttpRequestSpec) -> Result<Duration, MeNotifyError> {
        if !spec.timeout.is_finite() {
            return Err(MeNotifyError::HttpSpec("timeout must be a finite number".to_string()));
        }
        if spec.timeout.is_sign_negative() {
            return Err(MeNotifyError::HttpSpec("timeout must not be negative".to_string()));
        }
        if spec.timeout > Duration::MAX.as_secs_f64() {
            return Err(MeNotifyError::HttpSpec(format!("timeout {} is too large", spec.timeout)));
        }
        Ok(Duration::from_secs_f64(spec.timeout.max(0.001)))
    }

    /// Executes a blocking HTTP request.
    ///
    /// Arguments:
    /// * `spec` - HTTP request specification.
    ///
    /// Returns:
    /// * `Ok(MeNotifyHttpResponse)` on success.
    /// * `Err(MeNotifyError)` on validation or transport failure.
    pub fn request(spec: &MeNotifyHttpRequestSpec) -> Result<MeNotifyHttpResponse, MeNotifyError> {
        let mut builder = reqwest::blocking::Client::builder().timeout(Self::timeout(spec)?);
        if spec.insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let mut rb = builder.build().map_err(MeNotifyError::from)?.request(
            spec.method.parse::<reqwest::Method>().map_err(|err| MeNotifyError::HttpSpec(format!("invalid method '{}': {err}", spec.method)))?,
            &spec.url,
        );

        for (k, v) in &spec.headers {
            rb = rb.header(k, v);
        }
        if let Some(body) = &spec.body {
            rb = rb.body(body.clone());
        }

        let rsp = rb.send().map_err(MeNotifyError::from)?;
        let status = rsp.status().as_u16();
        let headers =
            rsp.headers().iter().map(|(k, v)| (k.as_str().to_lowercase(), v.to_str().unwrap_or_default().to_string())).collect::<BTreeMap<_, _>>();
        let body = rsp.text().map_err(MeNotifyError::from)?;
        let json = if spec.parse_json { serde_json::from_str::<serde_json::Value>(&body).ok() } else { None };

        Ok(MeNotifyHttpResponse { body, headers, json, ok: (200..300).contains(&status), status })
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct MeNotifyHttpGetOptions {
    pub body: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub insecure: bool,
    pub parse_json: bool,
    pub timeout: f64,
}

impl Default for MeNotifyHttpGetOptions {
    fn default() -> Self {
        Self { body: None, headers: BTreeMap::new(), insecure: false, parse_json: true, timeout: 30.0 }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct MeNotifyHttpRequestSpec {
    pub body: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub insecure: bool,
    pub method: String,
    pub parse_json: bool,
    pub timeout: f64,
    pub url: String,
}

impl Default for MeNotifyHttpRequestSpec {
    fn default() -> Self {
        Self { body: None, headers: BTreeMap::new(), insecure: false, method: "GET".to_string(), parse_json: true, timeout: 30.0, url: String::new() }
    }
}

impl MeNotifyHttpRequestSpec {
    /// Builds a GET request specification from URL and optional JSON options.
    ///
    /// Arguments:
    /// * `url` - Target URL.
    /// * `opts` - Optional JSON object with GET options.
    ///
    /// Returns:
    /// * `Ok(MeNotifyHttpRequestSpec)` on success.
    /// * `Err(MeNotifyError)` when options are invalid.
    pub fn from_get(url: String, opts: Option<serde_json::Value>) -> Result<Self, MeNotifyError> {
        let opts = opts
            .map(serde_json::from_value::<MeNotifyHttpGetOptions>)
            .transpose()
            .map_err(|err| MeNotifyError::HttpSpec(format!("http.get(url, opts) invalid options: {err}")))?
            .unwrap_or_default();

        Ok(Self {
            body: opts.body,
            headers: opts.headers,
            insecure: opts.insecure,
            method: "GET".to_string(),
            parse_json: opts.parse_json,
            timeout: opts.timeout,
            url,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct MeNotifyHttpResponse {
    pub body: String,
    pub headers: BTreeMap<String, String>,
    pub json: Option<serde_json::Value>,
    pub ok: bool,
    pub status: u16,
}
