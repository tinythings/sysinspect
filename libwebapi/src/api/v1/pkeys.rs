use crate::{MasterInterfaceType, api::v1::TAG_RSAKEYS, keystore::get_webapi_keystore};
use actix_web::{HttpResponse, Responder, post, web};
use serde::Serialize;
use utoipa::ToSchema;

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
