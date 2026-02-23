use crate::MasterInterfaceType;
use actix_files::NamedFile;
use actix_web::Result as ActixResult;
use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct StoreMetaResponse {
    pub sha256: String,
    pub size_bytes: u64,
    pub fmode: u32,
    pub created_unix: u64,
    pub expires_unix: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/store/{sha256}",
    tag = "Datastore",
    params(
        ("sha256" = String, Path, description = "SHA256 of the stored object")
    ),
    responses(
        (status = 200, description = "Metadata for object", body = StoreMetaResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Datastore error")
    )
)]
#[get("/store/{sha256}")]
pub async fn store_meta_handler(master: web::Data<MasterInterfaceType>, sha256: web::Path<String>) -> impl Responder {
    let ds = {
        let m = master.lock().await;
        m.datastore().await
    };

    let ds = ds.lock().await;

    match ds.meta(&sha256) {
        Ok(Some(meta)) => HttpResponse::Ok().json(StoreMetaResponse {
            sha256: meta.sha256,
            size_bytes: meta.size_bytes,
            fmode: meta.fmode,
            created_unix: meta.created_unix,
            expires_unix: meta.expires_unix,
        }),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/store/{sha256}/blob",
    tag = "Datastore",
    params(
        ("sha256" = String, Path, description = "SHA256 of the stored object")
    ),
    responses(
        (status = 200, description = "Binary blob"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Datastore error")
    )
)]
#[get("/store/{sha256}/blob")]
pub async fn store_blob_handler(master: web::Data<MasterInterfaceType>, sha256: web::Path<String>) -> ActixResult<NamedFile> {
    let ds = {
        let m = master.lock().await;
        m.datastore().await
    };

    let ds = ds.lock().await;
    let path = ds.uri(&sha256);

    if !path.exists() {
        return Err(actix_web::error::ErrorNotFound("blob not found"));
    }

    Ok(NamedFile::open(path)?)
}
