use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use indexmap::IndexMap;
use libmodcore::{response::ModResponse, runtime};
use reqwest::Method;
use reqwest::blocking::{Client, ClientBuilder, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, time::Duration};

#[derive(Debug, Default, Deserialize)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    kind: Option<String>,
    username: Option<String>,
    password: Option<String>,
    token: Option<String>,
    header: Option<String>,
    value: Option<String>,
    param: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TlsConfig {
    ca_file: Option<String>,
    client_cert_file: Option<String>,
    client_key_file: Option<String>,
    insecure_skip_verify: Option<bool>,
    insecure_skip_hostname_verify: Option<bool>,
}

#[derive(Debug, Default)]
pub struct RequestSpec {
    method: String,
    url: String,
    headers: IndexMap<String, Value>,
    query: IndexMap<String, Value>,
    body: Option<Value>,
    auth: AuthConfig,
    tls: TlsConfig,
    timeout_secs: u64,
    ok_status: Vec<u16>,
}

#[derive(Debug)]
pub struct HttpModule<'a> {
    rt: &'a runtime::ModRequest,
}

impl RequestSpec {
    /// Build request specification from the module runtime request.
    pub fn from_request(rt: &runtime::ModRequest) -> Result<Self, String> {
        let method = runtime::get_arg(rt, "method");
        let url = runtime::get_arg(rt, "url");
        if method.is_empty() {
            return Err("Missing method".to_string());
        }
        if url.is_empty() {
            return Err("Missing url".to_string());
        }

        Ok(Self {
            method,
            url,
            headers: Self::object_arg(rt, "headers"),
            query: Self::object_arg(rt, "query"),
            body: Self::value_arg(rt, "body"),
            auth: Self::struct_arg(rt, "auth"),
            tls: Self::struct_arg(rt, "tls"),
            timeout_secs: rt.get_arg("timeout").and_then(|v| v.as_int()).and_then(|n| u64::try_from(n).ok()).unwrap_or(30),
            ok_status: Self::ok_status(rt),
        })
    }

    /// Convert an object argument into a string/value map.
    fn object_arg(rt: &runtime::ModRequest, key: &str) -> IndexMap<String, Value> {
        Self::value_arg(rt, key).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }

    /// Convert a structured argument into a typed object.
    fn struct_arg<T: for<'de> Deserialize<'de> + Default>(rt: &runtime::ModRequest, key: &str) -> T {
        Self::value_arg(rt, key).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }

    /// Get a raw JSON value argument.
    fn value_arg(rt: &runtime::ModRequest, key: &str) -> Option<Value> {
        rt.get_arg(key).map(Value::from)
    }

    /// Parse allowed non-2xx status codes.
    fn ok_status(rt: &runtime::ModRequest) -> Vec<u16> {
        match Self::value_arg(rt, "ok-status") {
            Some(Value::Array(items)) => items.into_iter().filter_map(|v| v.as_u64().and_then(|n| u16::try_from(n).ok())).collect(),
            Some(Value::Number(n)) => n.as_u64().and_then(|n| u16::try_from(n).ok()).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Convert the configured HTTP method to reqwest method.
    fn method(&self) -> Result<Method, String> {
        Method::from_bytes(self.method.as_bytes()).map_err(|e| format!("Invalid method '{}': {e}", self.method))
    }

    /// Convert request headers to reqwest headers.
    fn headers(&self) -> Result<HeaderMap, String> {
        let mut out = HeaderMap::new();
        for (name, value) in &self.headers {
            let name = HeaderName::try_from(name.as_str()).map_err(|e| format!("Invalid header name '{name}': {e}"))?;
            let value = HeaderValue::from_str(&scalar_to_string(value).ok_or_else(|| format!("Header '{name}' must be a scalar value"))?)
                .map_err(|e| format!("Invalid header value for '{name}': {e}"))?;
            out.insert(name, value);
        }
        Ok(out)
    }

    /// Convert query-string parameters to reqwest format.
    pub(crate) fn query(&self) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for (key, value) in &self.query {
            out.push((key.clone(), scalar_to_string(value).ok_or_else(|| format!("Query parameter '{key}' must be a scalar value"))?));
        }
        Ok(out)
    }

    /// Check if the given status should be treated as success.
    fn is_ok_status(&self, status: u16) -> bool {
        self.ok_status.contains(&status)
    }

    /// Create a request specification with query values for unit tests.
    #[cfg(test)]
    pub fn with_query(query: IndexMap<String, Value>) -> Self {
        Self { query, ..Self::default() }
    }
}

impl<'a> HttpModule<'a> {
    /// Create a new HTTP module instance.
    pub fn new(rt: &'a runtime::ModRequest) -> Self {
        Self { rt }
    }

    /// Run the HTTP module and return a generic module response.
    pub fn run(&self) -> ModResponse {
        match RequestSpec::from_request(self.rt) {
            Ok(spec) => self.execute(&spec),
            Err(err) => self.error_response(&err),
        }
    }

    /// Execute a resolved request specification.
    fn execute(&self, spec: &RequestSpec) -> ModResponse {
        let method = match spec.method() {
            Ok(method) => method,
            Err(err) => return self.error_response(&err),
        };
        let client = match self.client(spec) {
            Ok(client) => client,
            Err(err) => return self.error_response(&err),
        };
        let request = match self.request(&client, spec, method) {
            Ok(request) => request,
            Err(err) => return self.error_response(&err),
        };

        match request.send() {
            Ok(response) => self.response(response, spec),
            Err(err) => self.error_response(&format!("HTTP request error: {err}")),
        }
    }

    /// Build reqwest client with TLS settings.
    fn client(&self, spec: &RequestSpec) -> Result<Client, String> {
        let mut builder = ClientBuilder::new().timeout(Duration::from_secs(spec.timeout_secs));
        builder = self.with_ca(builder, &spec.tls)?;
        builder = self.with_identity(builder, &spec.tls)?;
        if spec.tls.insecure_skip_verify.unwrap_or(false) {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if spec.tls.insecure_skip_hostname_verify.unwrap_or(false) {
            builder = builder.danger_accept_invalid_hostnames(true);
        }
        builder.build().map_err(|e| format!("Unable to build HTTP client: {e}"))
    }

    /// Attach custom CA trust.
    fn with_ca(&self, builder: ClientBuilder, tls: &TlsConfig) -> Result<ClientBuilder, String> {
        match &tls.ca_file {
            Some(path) => {
                let pem = fs::read(path).map_err(|e| format!("Unable to read CA file '{path}': {e}"))?;
                let cert = reqwest::Certificate::from_pem(&pem).map_err(|e| format!("Unable to parse CA file '{path}': {e}"))?;
                Ok(builder.add_root_certificate(cert))
            }
            None => Ok(builder),
        }
    }

    /// Attach mTLS client identity.
    fn with_identity(&self, builder: ClientBuilder, tls: &TlsConfig) -> Result<ClientBuilder, String> {
        match &tls.client_cert_file {
            Some(cert_path) => {
                let key_path =
                    tls.client_key_file.as_deref().ok_or_else(|| "tls.client_key_file is required when tls.client_cert_file is set".to_string())?;
                let cert = fs::read(cert_path).map_err(|e| format!("Unable to read client cert file '{cert_path}': {e}"))?;
                let key = fs::read(key_path).map_err(|e| format!("Unable to read client key file '{key_path}': {e}"))?;
                let identity = reqwest::Identity::from_pkcs8_pem(&cert, &key)
                    .map_err(|e| format!("Unable to parse client identity '{cert_path}': {e}"))?;
                Ok(builder.identity(identity))
            }
            None => Ok(builder),
        }
    }

    /// Build request with headers, query, auth and body.
    fn request(&self, client: &Client, spec: &RequestSpec, method: Method) -> Result<RequestBuilder, String> {
        let headers = spec.headers()?;
        let mut query = spec.query()?;
        let request = client.request(method, &spec.url).headers(headers);
        let request = self.with_auth(request, &spec.auth, &mut query)?;
        let request = if query.is_empty() { request } else { request.query(&query) };
        Ok(self.with_body(request, spec.body.as_ref()))
    }

    /// Apply authentication to the outgoing request.
    fn with_auth(
        &self, mut request: RequestBuilder, auth: &AuthConfig, query: &mut Vec<(String, String)>,
    ) -> Result<RequestBuilder, String> {
        match auth.kind.as_deref().unwrap_or("").trim() {
            "" => Ok(request),
            "bearer" => {
                let token = auth.token.as_deref().filter(|s| !s.is_empty()).ok_or_else(|| "auth.token is required for bearer auth".to_string())?;
                request = request.bearer_auth(token);
                Ok(request)
            }
            "basic" => {
                let username = auth.username.as_deref().ok_or_else(|| "auth.username is required for basic auth".to_string())?;
                request = request.basic_auth(username, auth.password.clone());
                Ok(request)
            }
            "header" => {
                let header = auth.header.as_deref().ok_or_else(|| "auth.header is required for header auth".to_string())?;
                let value = auth.value.as_deref().ok_or_else(|| "auth.value is required for header auth".to_string())?;
                Ok(request.header(header, value))
            }
            "query" => {
                let param = auth.param.as_deref().ok_or_else(|| "auth.param is required for query auth".to_string())?;
                let value = auth.value.as_deref().ok_or_else(|| "auth.value is required for query auth".to_string())?;
                query.push((param.to_string(), value.to_string()));
                Ok(request)
            }
            other => Err(format!("Unsupported auth type: {other}")),
        }
    }

    /// Attach request body.
    fn with_body(&self, request: RequestBuilder, body: Option<&Value>) -> RequestBuilder {
        match body {
            Some(Value::Object(_)) | Some(Value::Array(_)) => request.json(body.unwrap()),
            Some(Value::String(text)) => request.body(text.clone()),
            Some(value) => request.body(value.to_string()),
            None => request,
        }
    }

    /// Convert reqwest response to module response.
    fn response(&self, response: reqwest::blocking::Response, spec: &RequestSpec) -> ModResponse {
        let status = response.status();
        let url = response.url().to_string();
        let headers = response_headers(response.headers());
        match response.bytes() {
            Ok(body) => self.ok_response(url, status.as_u16(), headers, response_body(&body), status.is_success() || spec.is_ok_status(status.as_u16())),
            Err(err) => self.error_response(&format!("Unable to read HTTP response body: {err}")),
        }
    }

    /// Build a successful or failed module response from HTTP payload.
    fn ok_response(&self, url: String, status: u16, headers: IndexMap<String, Value>, body: Value, ok: bool) -> ModResponse {
        let mut res = ModResponse::new();
        res.set_retcode(if ok { 0 } else { 1 });
        let message = if ok { "HTTP request completed".to_string() } else { format!("HTTP request failed with status {status}") };
        res.set_message(&message);
        let _ = res.set_data(json!({"url": url, "status": status, "ok": ok, "headers": headers, "body": body}));
        res
    }

    /// Build an error module response.
    fn error_response(&self, message: &str) -> ModResponse {
        let mut res = ModResponse::new();
        res.set_retcode(1);
        res.set_message(message);
        res
    }
}

/// Convert a scalar JSON value to string.
pub fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some(String::new()),
        _ => None,
    }
}

/// Convert response headers to a JSON object.
fn response_headers(headers: &HeaderMap) -> IndexMap<String, Value> {
    headers.iter().map(|(name, value)| (name.to_string(), Value::String(value.to_str().unwrap_or_default().to_string()))).collect()
}

/// Convert response body to text/json/base64 object.
pub fn response_body(bytes: &[u8]) -> Value {
    if bytes.is_empty() {
        return json!({});
    }

    let mut out = serde_json::Map::new();
    if let Ok(text) = std::str::from_utf8(bytes) {
        out.insert("text".to_string(), Value::String(text.to_string()));
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            out.insert("json".to_string(), parsed);
        }
    } else {
        out.insert("base64".to_string(), Value::String(BASE64.encode(bytes)));
    }
    Value::Object(out)
}
