use crate::{MasterInterfaceType, api::v1::TAG_RSAKEYS, keystore::get_webapi_keystore, sessions::get_session_store};
use actix_web::{HttpResponse, Responder, post, web};
use base64::Engine;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Push to push a user public key to store on the server.
#[derive(Deserialize, ToSchema)]
pub struct PubKeyRequest {
    pub sid_cipher: String,
    pub key: String,
}

#[derive(Serialize, ToSchema)]
pub struct PubKeyResponse {
    pub message: String,
}

#[derive(Serialize, ToSchema)]
pub struct PubKeyError {
    pub error: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/pushkey",
    request_body = PubKeyRequest,
    operation_id = "pushkey",
    tag = TAG_RSAKEYS,
    description = "Push a public key for a user. Requires an authenticated session ID.",
    responses(
        (status = 200, description = "Public key saved successfully", body = PubKeyResponse, example = json!({"message": "Public key saved successfully"})),
        (status = 400, description = "Bad Request", body = PubKeyError, example = json!({"error": "Invalid session ID"}))
    )
)]
#[post("/api/v1/pushkey")]
pub async fn pushkey_handler(master: web::Data<MasterInterfaceType>, body: web::Json<PubKeyRequest>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;
    let keystore = match get_webapi_keystore(cfg) {
        Ok(path) => path,
        Err(err) => {
            return HttpResponse::BadRequest().json(PubKeyError { error: format!("Internal error. Failed to init keystore: {err}") });
        }
    };
    let sid = match base64::engine::general_purpose::STANDARD.decode(&body.sid_cipher) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                return HttpResponse::BadRequest().json(PubKeyError { error: "Invalid base64 encoding in sid_cipher".to_string() });
            }
        },
        Err(_) => {
            return HttpResponse::BadRequest().json(PubKeyError { error: "Failed to decode base64 sid_cipher".to_string() });
        }
    };

    // Check if API is in dev mode or SID is valid
    let uid = get_session_store().lock().unwrap().uid(&sid);
    if !cfg.api_devmode() && uid.is_none() {
        return HttpResponse::BadRequest().json(PubKeyError { error: "Invalid session ID".to_string() });
    }

    if let Err(err) = keystore.save_key(&uid.unwrap_or_else(|| "developer".to_string()), &body.key) {
        return HttpResponse::BadRequest().json(PubKeyError { error: format!("Failed to save public key: {err}") });
    }

    HttpResponse::Ok().json(PubKeyResponse { message: "Public key saved successfully".to_string() })
}

#[derive(Serialize, ToSchema)]
pub struct MasterKeyResponse {
    pub key: String,
}

#[derive(Serialize, ToSchema)]
pub struct MasterKeyError {
    pub error: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/masterkey",
    tag = TAG_RSAKEYS,
    description = "Retrieve the master public key from the keystore.",
    operation_id = "masterKey",
    responses(
        (status = 200, description = "Public key operations", body = MasterKeyResponse),
        (status = 400, description = "Error retrieving master key", body = MasterKeyError)
    )
)]
#[post("/api/v1/masterkey")]
pub async fn masterkey_handler(master: web::Data<MasterInterfaceType>) -> impl Responder {
    let master = master.lock().await;
    let cfg = master.cfg().await;

    let keystore = match get_webapi_keystore(cfg) {
        Ok(path) => path,
        Err(err) => {
            return HttpResponse::BadRequest().json(MasterKeyError { error: format!("Internal error. Failed to init keystore: {err}") });
        }
    };

    match keystore.get_master_key() {
        Ok(key) => HttpResponse::Ok().json(MasterKeyResponse { key }),
        Err(err) => HttpResponse::BadRequest().json(MasterKeyError { error: format!("Failed to retrieve master key: {err}") }),
    }
}
