use crate::{MasterInterfaceType, api::v1::TAG_SYSTEM, pamauth, sessions::get_session_store};
use actix_web::{HttpResponse, Responder, post, web};
use libsysinspect::cfg::mmconf::AuthMethod::Pam;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Serialize)]
pub struct HealthInfo {
    pub telemetry_enabled: bool,
    pub scheduler_tasks: usize,
    pub api_version: String,
}

#[derive(ToSchema, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub info: HealthInfo,
}

#[utoipa::path(
    post,
    path = "/api/v1/health",
    tag = TAG_SYSTEM,
    operation_id = "healthCheck",
    description = "Checks the health of the SysInspect API. Returns basic information about the API status, telemetry, and scheduler tasks.",
    responses(
        (status = 200, description = "Health status", body = HealthResponse, example = json!({
            "status": "healthy",
            "info": {
                "telemetry_enabled": true,
                "scheduler_tasks": 5,
                "api_version": "0.1.0"
            }
        })),
        (status = 500, description = "Internal Server Error", body = HealthResponse,
            example = json!({
                "status": "unhealthy",
                "info": {
                    "telemetry_enabled": false,
                    "scheduler_tasks": 0,
                    "api_version": "0.1.0"
                }
            })
        )
    )
)]
#[post("/api/v1/health")]
pub async fn health_handler(master: web::Data<MasterInterfaceType>, _body: ()) -> impl Responder {
    let lock = master.lock().await;
    let cfg = lock.cfg().await;

    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "info": {
            "telemetry.enabled": cfg.telemetry_enabled(),
            "scheduler.tasks": cfg.scheduler().len(),
            "api.version": cfg.api_version(),
        }
    }))
}

#[derive(ToSchema, Deserialize, Serialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

impl AuthRequest {
    pub fn pam_auth(username: String, password: String) -> Result<String, String> {
        pamauth::authenticate(&username, &password).map_err(|err| format!("Authentication failed: {err}"))?;
        get_session_store().lock().unwrap().open(username.clone()).map_err(|e| format!("Session error: {e}"))
    }
}

#[derive(ToSchema, Deserialize, Serialize)]
pub struct AuthResponse {
    pub status: String,
    pub access_token: String,
    pub token_type: String,
    pub error: String,
}

impl AuthResponse {
    pub(crate) fn error(error: &str) -> Self {
        AuthResponse { status: "error".into(), access_token: String::new(), token_type: String::new(), error: error.into() }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/authenticate",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Authentication successful. Returns a bearer token.",
             body = AuthResponse, example = json!({"status": "authenticated", "access_token": "session-token", "token_type": "Bearer", "error": ""})),
        (status = 400, description = "Bad Request. Returned if payload is missing, invalid, or credentials are incorrect.",
             body = AuthResponse, example = json!({"status": "error", "access_token": "", "token_type": "", "error": "Invalid payload"}))),
    tag = TAG_SYSTEM,
    operation_id = "authenticateUser",
    description = "Authenticates a user using configured authentication method and returns a bearer token for subsequent HTTPS JSON requests.",
)]
#[post("/api/v1/authenticate")]
pub async fn authenticate_handler(master: web::Data<MasterInterfaceType>, body: web::Json<AuthRequest>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;
    if cfg.api_devmode() {
        log::warn!("Web API development auth bypass is enabled, returning static token.");
        return match get_session_store().lock().unwrap().open_with_sid("dev".into(), "dev-token".into()) {
            Ok(token) => HttpResponse::Ok().json(AuthResponse {
                status: "authenticated".into(),
                access_token: token,
                token_type: "Bearer".into(),
                error: String::new(),
            }),
            Err(err) => HttpResponse::BadRequest().json(AuthResponse::error(&format!("Session error: {err}"))),
        };
    }

    if body.username.trim().is_empty() || body.password.trim().is_empty() {
        return HttpResponse::BadRequest().json(AuthResponse::error("Username and/or password are required"));
    }

    if cfg.api_auth() == Pam {
        match AuthRequest::pam_auth(body.username.clone(), body.password.clone()) {
            Ok(token) => HttpResponse::Ok().json(AuthResponse {
                status: "authenticated".into(),
                access_token: token,
                token_type: "Bearer".into(),
                error: String::new(),
            }),
            Err(err) => HttpResponse::BadRequest().json(AuthResponse::error(&err)),
        }
    } else {
        HttpResponse::BadRequest().json(AuthResponse::error("PAM authentication is not enabled"))
    }
}
