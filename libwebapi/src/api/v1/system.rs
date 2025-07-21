use crate::{MasterInterfaceArc, pamauth, sessions::get_session_store};
use actix_web::{HttpResponse, Responder, post, web};
use libsysinspect::cfg::mmconf::AuthMethod::Pam;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[utoipa::path(
    post,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Health status", body = String)
    )
)]
#[post("/api/v1/health")]
pub async fn health_handler(master: web::Data<MasterInterfaceArc>, _body: ()) -> impl Responder {
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
    pub fn authenticate(username: String, password: String) -> Result<String, String> {
        pamauth::authenticate(&username, &password).map_err(|err| format!("Authentication failed: {err}"))?;
        Ok(get_session_store().lock().unwrap().open(username.clone()))
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/authenticate",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Success", body = String),
        (status = 400, description = "Bad Request")
    )
)]
#[post("/api/v1/authenticate")]
pub async fn authenticate_handler(master: web::Data<MasterInterfaceArc>, body: web::Json<AuthRequest>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;
    if cfg.api_devmode() {
        return HttpResponse::Ok().json(serde_json::json!({"status": "authenticated", "token": "dev-token"}));
    }

    if cfg.api_auth() == Pam {
        match AuthRequest::authenticate(body.username.clone(), body.password.clone()) {
            Ok(sid) => HttpResponse::Ok().json(serde_json::json!({
                "status": "authenticated",
                "sid": sid,
            })),
            Err(err) => HttpResponse::BadRequest().json(serde_json::json!({"error": err})),
        }
    } else {
        HttpResponse::BadRequest().json(serde_json::json!({"error": "PAM authentication is not enabled"}))
    }
}
