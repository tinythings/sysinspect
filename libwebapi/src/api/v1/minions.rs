pub use crate::api::v1::system::health_handler;
use crate::{MasterInterfaceType, api::v1::TAG_MINIONS, sessions::get_session_store};
use actix_web::{
    Result, post,
    web::{Data, Json},
};
use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct QueryRequest {
    pub sid: String,
    pub model: String,
    pub query: String,
    pub traits: String,
    pub mid: String,
    pub context: HashMap<String, String>,
}

impl QueryRequest {
    /// Validate the session and convert the request into the internal query format.
    pub fn to_query(&self) -> Result<String, SysinspectError> {
        if self.sid.trim().is_empty() {
            return Err(SysinspectError::WebAPIError("Session ID cannot be empty".to_string()));
        }

        let mut sessions = get_session_store().lock().unwrap();
        match sessions.uid(&self.sid) {
            Some(_) => Ok(format!(
                "{};{};{};{};{}",
                self.model,
                self.query,
                self.traits,
                self.mid,
                self.context.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",")
            )),
            None => {
                log::debug!("Session {} is missing or expired", self.sid);
                Err(SysinspectError::WebAPIError("Invalid or expired session".to_string()))
            }
        }
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
async fn query_handler(master: Data<MasterInterfaceType>, body: Json<QueryRequest>) -> Result<Json<QueryResponse>> {
    let mut master = master.lock().await;
    let query = match body.to_query() {
        Ok(q) => q,
        Err(e) => {
            use actix_web::http::StatusCode;
            let err_body = Json(QueryError { status: "error".to_string(), error: e.to_string() });
            return Err(actix_web::error::InternalError::new(err_body, StatusCode::BAD_REQUEST).into());
        }
    };

    match master.query(query).await {
        Ok(()) => Ok(Json(QueryResponse { status: "success".to_string(), message: "Query executed successfully".to_string() })),
        Err(err) => Ok(Json(QueryResponse { status: "error".to_string(), message: err.to_string() })),
    }
}
