pub use crate::api::v1::system::health_handler;
use crate::{MasterInterfaceType, api::v1::TAG_MINIONS, keystore::get_webapi_keystore, sessions::get_session_store};
use actix_web::{post, web};
use base64::{Engine, engine::general_purpose::STANDARD};
use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig};
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::secretbox::Nonce;
use std::{collections::HashMap, fmt::Display, str::from_utf8};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct QueryPayloadRequest {
    pub model: String,
    pub query: String,
    pub traits: String,
    pub mid: String,
    pub context: HashMap<String, String>,
}

impl QueryPayloadRequest {
    pub fn to_query(&self) -> String {
        format!(
            "{};{};{};{};{}",
            self.model,
            self.query,
            self.traits,
            self.mid,
            self.context.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",")
        )
    }
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct QueryRequest {
    pub sid_rsa: String, // RSA encrypted session ID
    pub nonce: String,   // Nonce for symmetric encryption
    pub payload: String, // Base64-encoded, symmetric-encrypted JSON payload
}

impl QueryRequest {
    pub fn to_query_request(&self, cfg: &MasterConfig) -> Result<QueryPayloadRequest, SysinspectError> {
        if self.sid_rsa.is_empty() {
            return Err(SysinspectError::RSAError("Session ID cannot be empty".to_string()));
        }

        let keystore = get_webapi_keystore(cfg)?;
        let mut sessions = get_session_store().lock().unwrap();
        let sid = keystore.decrypt_user_data(
            &STANDARD.decode(&self.sid_rsa).map_err(|e| SysinspectError::RSAError(format!("Failed to decode sid_rsa from base64: {e}")))?,
        )?;
        sessions.decrypt(
            from_utf8(&sid).map_err(|_| SysinspectError::WebAPIError("Session ID is not valid UTF-8".to_string()))?,
            &Nonce::from_slice(
                &STANDARD.decode(&self.nonce).map_err(|e| SysinspectError::WebAPIError(format!("Failed to decode nonce from base64: {e}")))?,
            )
            .ok_or(SysinspectError::WebAPIError("Invalid nonce length".to_string()))?
            .0,
            &STANDARD.decode(&self.payload).map_err(|e| SysinspectError::WebAPIError(format!("Failed to decode payload from base64: {e}")))?,
        )
    }
}
#[derive(Serialize, ToSchema)]
pub struct QueryResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QueryError {
    pub status: String,
    pub error: String,
}

impl Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Query error. Status: {}, message: {})", self.status, self.error)
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/query",
    request_body = QueryRequest,
    tag = TAG_MINIONS,
    responses(
        (status = 200, description = "Success", body = QueryResponse),
        (status = 400, description = "Bad Request", body = QueryError)
    )
)]
#[post("/api/v1/query")]
async fn query_handler(master: web::Data<MasterInterfaceType>, body: web::Json<QueryRequest>) -> actix_web::Result<web::Json<QueryResponse>> {
    let mut master = master.lock().await;
    let cfg = master.cfg().await;
    let qpr = match body.to_query_request(cfg) {
        Ok(q) => q,
        Err(e) => {
            use actix_web::http::StatusCode;
            let err_body = web::Json(QueryError { status: "error".to_string(), error: e.to_string() });
            return Err(actix_web::error::InternalError::new(err_body, StatusCode::BAD_REQUEST).into());
        }
    };

    match master.query(qpr.to_query()).await {
        Ok(()) => Ok(web::Json(QueryResponse { status: "success".to_string(), message: "Query executed successfully".to_string() })),
        Err(err) => Ok(web::Json(QueryResponse { status: "error".to_string(), message: err.to_string() })),
    }
}
