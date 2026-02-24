use crate::MasterInterfaceType;
use actix_files::NamedFile;
use actix_web::{HttpRequest, Result as ActixResult};
use actix_web::{HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;
#[derive(Debug, Serialize, ToSchema)]
pub struct StoreMetaResponse {
    pub sha256: String,
    pub size_bytes: u64,
    pub fmode: u32,
    pub created_unix: u64,
    pub expires_unix: Option<u64>,
    pub fname: Option<String>,
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
            fname: meta.fname,
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

#[utoipa::path(
    post,
    path = "/store",
    tag = "Datastore",
    request_body(
        content = Vec<u8>,
        content_type = "application/octet-stream",
        description = "Raw bytes to store"
    ),
    responses(
        (status = 200, description = "Stored successfully", body = StoreMetaResponse),
        (status = 413, description = "Payload too large"),
        (status = 500, description = "Datastore error")
    )
)]
#[post("/store")]
pub async fn store_upload_handler(req: actix_web::HttpRequest, master: web::Data<MasterInterfaceType>, mut payload: web::Payload) -> impl Responder {
    // full path goes into fname (as you demanded)
    let origin = req.headers().get("X-Filename").and_then(|v| v.to_str().ok()).map(|s| s.to_string());

    let ds = {
        let m = master.lock().await;
        m.datastore().await
    };

    let tmp = match tempfile::NamedTempFile::new() {
        Ok(f) => f,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    let tmp_path = tmp.path().to_path_buf();

    let mut f = match tokio::fs::File::create(&tmp_path).await {
        Ok(f) => f,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    while let Some(chunk) = payload.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
        };
        if let Err(e) = f.write_all(&chunk).await {
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    }

    if let Err(e) = f.flush().await {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    drop(f);

    // store
    let mut meta = {
        let ds = ds.lock().await;
        match ds.add(&tmp_path) {
            Ok(m) => m,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidInput || e.kind() == std::io::ErrorKind::OutOfMemory {
                    return HttpResponse::PayloadTooLarge().body(e.to_string());
                }
                return HttpResponse::InternalServerError().body(e.to_string());
            }
        }
    };

    // overwrite fname with full path + persist to meta sidecar
    if let Some(origin) = origin {
        if origin.contains('\0') {
            return HttpResponse::BadRequest().body("invalid X-Filename");
        }

        meta.fname = Some(origin);

        // compute sidecar path: <shard>/<sha>.meta.json
        let meta_path = {
            let ds = ds.lock().await;
            let data_path = ds.uri(&meta.sha256);
            data_path.parent().unwrap().join(format!("{}.meta.json", meta.sha256))
        };

        // atomic rewrite
        let tmp_meta = meta_path.with_extension("meta.json.tmp");
        if let Err(e) = std::fs::write(&tmp_meta, serde_json::to_vec(&meta).unwrap()) {
            return HttpResponse::InternalServerError().body(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp_meta, &meta_path) {
            return HttpResponse::InternalServerError().body(e.to_string());
        }
    }

    drop(tmp);

    HttpResponse::Ok().json(StoreMetaResponse {
        sha256: meta.sha256,
        size_bytes: meta.size_bytes,
        fmode: meta.fmode,
        created_unix: meta.created_unix,
        expires_unix: meta.expires_unix,
        fname: meta.fname,
    })
}
