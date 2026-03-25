pub use crate::api::v1::system::health_handler;
#[cfg(test)]
#[path = "minions_ut.rs"]
mod minions_ut;
use crate::{MasterInterfaceType, api::v1::TAG_MINIONS, sessions::get_session_store};
use actix_web::{
    HttpRequest,
    Result, post,
    web::{Data, Json},
};
use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct QueryRequest {
    pub model: String,
    pub query: String,
    pub traits: String,
    pub mid: String,
    pub context: HashMap<String, String>,
}

impl QueryRequest {
    pub fn to_query(&self) -> Result<String, SysinspectError> {
        Ok(format!(
            "{};{};{};{};{}",
            self.model,
            self.query,
            self.traits,
            self.mid,
            self.context.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",")
        ))
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

pub(crate) async fn authorise_request(req: &HttpRequest) -> Result<String, SysinspectError> {
    let header = req
        .headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| SysinspectError::WebAPIError("Missing Authorization header".to_string()))?;
    let token = header
        .split_once(char::is_whitespace)
        .and_then(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer").then_some(token.trim()))
        .ok_or_else(|| SysinspectError::WebAPIError("Authorization header must use Bearer token".to_string()))?;
    if token.is_empty() {
        return Err(SysinspectError::WebAPIError("Bearer token cannot be empty".to_string()));
    }

    let mut sessions = get_session_store().lock().await;
    match sessions.uid(token) {
        Some(uid) => {
            sessions.ping(token);
            Ok(uid)
        }
        None => Err(SysinspectError::WebAPIError("Invalid or expired bearer token".to_string())),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/query",
    request_body = QueryRequest,
    tag = TAG_MINIONS,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Success", body = QueryResponse),
        (status = 400, description = "Bad Request", body = QueryError),
        (status = 401, description = "Unauthorized", body = QueryError)
    )
)]
#[post("/api/v1/query")]
async fn query_handler(req: HttpRequest, master: Data<MasterInterfaceType>, body: Json<QueryRequest>) -> Result<Json<QueryResponse>> {
    if let Err(e) = authorise_request(&req).await {
        use actix_web::http::StatusCode;
        let err_body = Json(QueryError { status: "error".to_string(), error: e.to_string() });
        return Err(actix_web::error::InternalError::new(err_body, StatusCode::UNAUTHORIZED).into());
    }

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
