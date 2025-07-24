use crate::{MasterInterfaceType, api::v1::TAG_SYSTEM, keystore::get_webapi_keystore, pamauth, sessions::get_session_store};
use actix_web::{HttpResponse, Responder, post, web};
use base64::{Engine, engine::general_purpose::STANDARD};
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
    /// Base64-encoded, RSA-encrypted JSON: {"username": "...", "password": "...", "pubkey": "..."}
    pub payload: String,
    pub pubkey: String,
}

impl AuthRequest {
    pub fn pam_auth(username: String, password: String) -> Result<String, String> {
        pamauth::authenticate(&username, &password).map_err(|err| format!("Authentication failed: {err}"))?;
        get_session_store().lock().unwrap().open(username.clone()).map_err(|e| format!("Session error: {e}"))
    }
}

#[derive(ToSchema, Deserialize, Serialize)]
pub struct AuthInnerRequest {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(ToSchema, Deserialize, Serialize)]
pub struct AuthResponse {
    pub status: String,
    pub sid_cipher: String,
    pub symkey_cipher: String,
    pub error: String,
}

impl AuthResponse {
    pub(crate) fn error(error: &str) -> Self {
        AuthResponse { status: "error".into(), sid_cipher: String::new(), symkey_cipher: String::new(), error: error.into() }
    }
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
    tag = TAG_SYSTEM,
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
pub async fn authenticate_handler(master: web::Data<MasterInterfaceType>, body: web::Json<AuthRequest>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;
    if cfg.api_devmode() {
        log::warn!("API is in development mode, returning static token!");
        return HttpResponse::Ok().json(AuthResponse {
            status: "authenticated".into(),
            sid_cipher: "dev-token".into(),
            symkey_cipher: String::new(),
            error: String::new(),
        });
    }

    if body.payload.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!(AuthResponse::error("Payload is missing")));
    }

    let payload = match STANDARD.decode(body.payload.as_bytes()) {
        Ok(d) => d,
        Err(_) => {
            log::debug!("Failed to decode payload, expecting base64-encoded encrypted data");
            return HttpResponse::BadRequest().json(serde_json::json!(AuthResponse::error("Invalid payload format")));
        }
    };

    let keystore = match get_webapi_keystore(cfg) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError().json(AuthResponse::error(&format!("Keystore error: {e}")));
        }
    };

    let decrypted = match keystore.decrypt_user_data(&payload) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to decrypt user data: {e}");
            return HttpResponse::BadRequest().json(AuthResponse::error(&format!("Decryption error: {e}")));
        }
    };

    let creds: AuthInnerRequest = match serde_json::from_slice(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to parse decrypted user data: {e}");
            return HttpResponse::BadRequest().json(AuthResponse::error(&format!("Invalid credentials format: {e}")));
        }
    };

    if creds.username.as_deref().unwrap_or("").is_empty() || creds.password.as_deref().unwrap_or("").is_empty() {
        return HttpResponse::BadRequest().json(AuthResponse::error("Username and/or password are required"));
    }

    if cfg.api_auth() == Pam {
        let uid = creds.username.unwrap();
        match AuthRequest::pam_auth(uid.clone(), creds.password.unwrap()) {
            Ok(sid) => {
                keystore.save_key(&uid, &body.pubkey).unwrap_or_else(|e| {
                    log::error!("Failed to save public key: {e}");
                });

                let mut session = get_session_store().lock().unwrap();
                match session.key(&sid) {
                    Some(key) => {
                        // Encrypt the session key with the user's RSA public key
                        let symkey_cipher = match keystore.encrypt_user_data(&uid, &hex::encode(&key.0)) {
                            Ok(enc) => STANDARD.encode(&enc),
                            Err(e) => {
                                log::error!("Failed to encrypt session key: {e}");
                                return HttpResponse::InternalServerError().json(AuthResponse::error("Failed to encrypt session key"));
                            }
                        };

                        // Encode the session ID as base64
                        let sid_cipher = match keystore.encrypt_user_data(&uid, &sid) {
                            Ok(enc) => STANDARD.encode(&enc),
                            Err(e) => {
                                log::error!("Failed to encrypt session ID: {e}");
                                return HttpResponse::InternalServerError().json(AuthResponse::error("Failed to encrypt session ID"));
                            }
                        };

                        HttpResponse::Ok().json(AuthResponse { status: "authenticated".into(), sid_cipher, symkey_cipher, error: String::new() })
                    }
                    None => HttpResponse::InternalServerError().json(AuthResponse::error("Session key not found")),
                }

                /*
                let mut session = get_session_store().lock().unwrap();
                let (nonce, ciphertext) = session.encrypt(&sid, &json!({"name": "john"})).unwrap();
                log::info!("Encrypted session data for user {}: nonce = {:?}, ciphertext = {:?}", uid, nonce, ciphertext);
                let res = session.decrypt::<serde_json::Value>(&sid, nonce.as_slice(), ciphertext.as_slice()).unwrap();
                log::info!("Decrypted session data for user {}: {:?}", uid, res);
                */
            }
            Err(err) => HttpResponse::BadRequest().json(AuthResponse::error(&err)),
        }
    } else {
        HttpResponse::BadRequest().json(AuthResponse::error("PAM authentication is not enabled"))
    }
}
