use crate::{MasterInterfaceArc, keystore::get_webapi_keystore, pamauth, sessions::get_session_store};
use actix_web::{HttpResponse, Responder, post, web};
use base64::{Engine, engine::general_purpose::STANDARD};
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
    /// Base64-encoded, RSA-encrypted JSON: {"username": "...", "password": "...", "pubkey": "..."}
    pub payload: String,
    pub pubkey: String,
}

impl AuthRequest {
    pub fn authenticate(username: String, password: String) -> Result<String, String> {
        pamauth::authenticate(&username, &password).map_err(|err| format!("Authentication failed: {err}"))?;
        Ok(get_session_store().lock().unwrap().open(username.clone()))
    }
}

#[derive(ToSchema, Deserialize, Serialize)]
struct InnerAuthRequest {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(ToSchema, Deserialize, Serialize)]
pub struct AuthResponse {
    pub status: String,
    pub sid: Option<String>,
    pub error: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/authenticate",
    request_body(
        content = AuthRequest,
        description = "Base64-encoded, RSA-encrypted JSON containing username and password. See description for details.",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Authentication successful. Returns a session ID (sid) if credentials are valid.",
         body = AuthResponse, example = json!({"status": "authenticated", "sid": "session-id"})),
        (status = 400, description = "Bad Request. Returned if payload is missing, invalid, or credentials are incorrect.",
         body = AuthResponse, example = json!({"status": "error", "sid": null, "error": "Invalid payload"}))),
    tag = "Authentication",
    operation_id = "authenticateUser",
    description =
        "Authenticates a user using configured authentication method. The payload \
        must be a base64-encoded, RSA-encrypted JSON object with username and \
        password fields as follows:\n\n\
        ```json\n\
        {\n\
          \"username\": \"darth_vader\",\n\
          \"password\": \"I am your father\",\n\
          \"pubkey\": \"...\"\n\
        }\n\
        ```\n\n\
        If the API is in development mode, it will return a static token without \
        actual authentication.",
)]
#[post("/api/v1/authenticate")]
pub async fn authenticate_handler(master: web::Data<MasterInterfaceArc>, body: web::Json<AuthRequest>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;
    if cfg.api_devmode() {
        log::warn!("API is in development mode, returning static token!");
        return HttpResponse::Ok().json(AuthResponse { status: "authenticated".into(), sid: Some("dev-token".into()), error: None });
    }

    if body.payload.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!(AuthResponse {
            status: "error".into(),
            sid: None,
            error: Some("Payload is missing".into())
        }));
    }

    let payload = match STANDARD.decode(body.payload.as_bytes()) {
        Ok(d) => d,
        Err(_) => {
            log::debug!("Failed to decode payload, expecting base64-encoded encrypted data");
            return HttpResponse::BadRequest().json(serde_json::json!(AuthResponse {
                status: "error".into(),
                sid: None,
                error: Some("Invalid payload format".into())
            }));
        }
    };

    let keystore = match get_webapi_keystore(cfg) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError().json(AuthResponse {
                status: "error".into(),
                sid: None,
                error: Some(format!("Keystore error: {e}")),
            });
        }
    };

    let decrypted = match keystore.decrypt_user_data(&payload) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to decrypt user data: {e}");
            return HttpResponse::BadRequest().json(AuthResponse {
                status: "error".into(),
                sid: None,
                error: Some(format!("Decryption error: {e}")),
            });
        }
    };

    let creds: InnerAuthRequest = match serde_json::from_slice(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to parse decrypted user data: {e}");
            return HttpResponse::BadRequest().json(AuthResponse {
                status: "error".into(),
                sid: None,
                error: Some(format!("Invalid credentials format: {e}")),
            });
        }
    };

    if creds.username.as_deref().unwrap_or("").is_empty() || creds.password.as_deref().unwrap_or("").is_empty() {
        return HttpResponse::BadRequest().json(AuthResponse {
            status: "error".into(),
            sid: None,
            error: Some("Username and/or password are required".into()),
        });
    }

    if cfg.api_auth() == Pam {
        match AuthRequest::authenticate(creds.username.unwrap(), creds.password.unwrap()) {
            Ok(sid) => {
                log::info!("User authenticated successfully, pubkey: {}", body.pubkey);
                HttpResponse::Ok().json(AuthResponse { status: "authenticated".into(), sid: Some(sid), error: None })
            }
            Err(err) => HttpResponse::BadRequest().json(AuthResponse { status: "error".into(), sid: None, error: Some(err) }),
        }
    } else {
        HttpResponse::BadRequest().json(AuthResponse { status: "error".into(), sid: None, error: Some("PAM authentication is not enabled".into()) })
    }
}
