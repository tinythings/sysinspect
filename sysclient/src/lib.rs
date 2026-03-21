use libcommon::SysinspectError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, collections::HashMap};

/// SysClient Configuration
/// This struct holds the configuration for the SysClient, including the root directory.
/// It can be extended in the future to include more configuration options.
///
/// # Fields
/// * `master_url` - The URL of the SysInspect master server.
#[derive(Debug, Clone)]
pub struct SysClientConfiguration {
    pub master_url: String,
}

impl SysClientConfiguration {
    fn client(&self) -> Client {
        Client::builder().user_agent("sysinspect-client/0.1.0").build().unwrap_or_else(|_| Client::new())
    }
}

impl Default for SysClientConfiguration {
    fn default() -> Self {
        SysClientConfiguration { master_url: "http://localhost:4202".to_string() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthResponse {
    pub status: String,
    pub sid: String,
    pub error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryRequest {
    pub sid: String,
    pub model: String,
    pub query: String,
    pub traits: String,
    pub mid: String,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelNameResponse {
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub maintainer: String,
    #[allow(clippy::type_complexity)]
    #[serde(rename = "entity-states")]
    pub entity_states: BTreeMap<String, Vec<(String, BTreeMap<String, String>)>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelResponse {
    pub model: ModelInfo,
}

/// SysClient is the main client for interacting with the SysInspect Web API.
/// It handles authentication and plain JSON request/response flows.
///
/// # Fields
/// * `cfg` - The configuration for the SysClient, which includes the master URL.
/// * `sid` - The session ID for the authenticated user.
#[derive(Debug, Clone)]
pub struct SysClient {
    cfg: SysClientConfiguration,
    sid: String,
}

impl SysClient {
    pub fn new(cfg: SysClientConfiguration) -> Self {
        SysClient { cfg, sid: String::new() }
    }

    /// Authenticate a user with the SysInspect system.
    /// This method sets up the client first and then performs authentication.
    ///
    /// # Arguments
    /// * `uid` - The user ID to authenticate.
    /// * `pwd` - The password for the user.
    ///
    ///  # Returns
    /// A `Result` that is `Ok(true)` if authentication is successful,
    /// or `Ok(false)` if authentication fails.
    /// If there is an error during the setup or authentication process, it returns an `Err(SysinspectError)`.
    pub async fn authenticate(&mut self, uid: &str, pwd: &str) -> Result<String, SysinspectError> {
        log::debug!("Authenticating user: {uid}");
        let response = self
            .cfg
            .client()
            .post(format!("{}/api/v1/authenticate", self.cfg.master_url.trim_end_matches('/')))
            .json(&AuthRequest { username: uid.to_string(), password: pwd.to_string() })
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication error: {e}")))?;
        let response = response
            .error_for_status()
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication error: {e}")))?
            .json::<AuthResponse>()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication decode error: {e}")))?;

        if response.status != "authenticated" || response.sid.trim().is_empty() {
            return Err(SysinspectError::MasterGeneralError(if response.error.is_empty() {
                "Authentication failed".to_string()
            } else {
                response.error
            }));
        }

        self.sid = response.sid;
        log::debug!("Authenticated user: {uid}, session ID: {}", self.sid);

        Ok(self.sid.clone())
    }

    /// Query the SysInspect system with a given query string.
    /// This method requires the client to be authenticated (i.e., `sid` must not be empty).
    ///
    /// # Arguments
    /// * `query` - The query string to send to the SysInspect system.
    ///
    /// # Returns
    /// A `Result` that is `Ok(String)` containing the response from the SysInspect system,
    /// or an `Err(SysinspectError)` if there is an error during the query process.
    ///
    /// # Errors
    /// * Returns `SysinspectError::MasterGeneralError` if the client is not authenticated (i.e., `sid` is empty),
    /// * Returns `SysinspectError::MasterGeneralError` if there is an error during the query process, such as network issues or server errors.
    ///
    /// This function constructs a plain JSON payload containing the session ID and query,
    /// sends it to the `query_handler` API, and returns the decoded JSON response.
    pub async fn query(
        &self, model: &str, query: &str, traits: &str, mid: &str, context: Value,
    ) -> Result<QueryResponse, SysinspectError> {
        if self.sid.is_empty() {
            return Err(SysinspectError::MasterGeneralError("Client is not authenticated".to_string()));
        }

        let query_request = QueryRequest {
            sid: self.sid.clone(),
            model: model.to_string(),
            query: query.to_string(),
            traits: traits.to_string(),
            mid: mid.to_string(),
            context: Self::context_map(context)?,
        };

        let response = self
            .cfg
            .client()
            .post(format!("{}/api/v1/query", self.cfg.master_url.trim_end_matches('/')))
            .json(&query_request)
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Query error: {e}")))?
            .error_for_status()
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Query error: {e}")))?
            .json::<QueryResponse>()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Query decode error: {e}")))?;

        Ok(response)
    }

    /// Retrieve the list of available models from the SysInspect system.
    /// This method requires the client to be authenticated.
    /// # Returns
    /// A `Result` that is `Ok(ModelNameResponse)` containing the list of models,
    /// or an `Err(SysinspectError)` if there is an error during the retrieval process.
    /// # Errors
    /// * Returns `SysinspectError::MasterGeneralError` if there is an error during the retrieval process, such as network issues or server errors.
    ///
    /// Calls the `list_models` API to fetch available models from the SysInspect system.
    /// Returns a `ModelNameResponse` containing the list of models on success, or a `SysinspectError` if the API call fails.
    /// This enables the caller to access the models provided by the SysInspect system.
    pub async fn models(&self) -> Result<ModelNameResponse, SysinspectError> {
        self.cfg
            .client()
            .get(format!("{}/api/v1/model/names", self.cfg.master_url.trim_end_matches('/')))
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to list models: {e}")))?
            .error_for_status()
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to list models: {e}")))?
            .json::<ModelNameResponse>()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to decode model list: {e}")))
    }

    pub async fn model_descr(&self, name: &str) -> Result<ModelResponse, SysinspectError> {
        self.cfg
            .client()
            .get(format!("{}/api/v1/model/descr", self.cfg.master_url.trim_end_matches('/')))
            .query(&[("name", name)])
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to get model details: {e}")))?
            .error_for_status()
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to get model details: {e}")))?
            .json::<ModelResponse>()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to decode model details: {e}")))
    }

    fn context_map(context: Value) -> Result<HashMap<String, String>, SysinspectError> {
        let Value::Object(map) = context else {
            return Err(SysinspectError::SerializationError("Query context must be a JSON object".to_string()));
        };

        Ok(map
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    Value::String(text) => text,
                    other => other.to_string(),
                };
                (key, value)
            })
            .collect())
    }
}
