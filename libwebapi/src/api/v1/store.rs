use std::path::{Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD};
use crate::{MasterInterfaceType, api::v1::minions::authorise_request, sessions::get_session_store};
use actix_files::NamedFile;
use actix_web::Result as ActixResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use libdatastore::resources::DataItemMeta;
use libsysinspect::rsa::keys::{RsaKey, key_from_file, verify_sign};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::task;
use utoipa::ToSchema;

const MINION_AUTH_SKEW_SECS: u64 = 300;

#[derive(Debug, Serialize, ToSchema)]
pub struct StoreMetaResponse {
    pub sha256: String,
    pub size_bytes: u64,
    pub fmode: u32,
    pub created_unix: u64,
    pub expires_unix: Option<u64>,
    pub fname: Option<String>,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct StoreResolveQuery {
    pub fname: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StoreListQuery {
    pub prefix: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StoreErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StoreMinionAuthResponse {
    pub status: String,
    pub access_token: String,
    pub token_type: String,
    pub error: String,
}

fn unauthorised_store_error(err: libcommon::SysinspectError) -> actix_web::Error {
    let msg = err.to_string();
    actix_web::error::InternalError::from_response(err, HttpResponse::Unauthorized().json(StoreErrorResponse { error: msg })).into()
}

fn minion_auth_material(method: &str, path: &str, query: &str, timestamp: &str, body_sha256: &str) -> String {
    format!("{}\n{}\n{}\n{}\n{}", method, path, query, timestamp, body_sha256)
}

async fn verify_minion_bootstrap(req: &HttpRequest, master: &web::Data<MasterInterfaceType>) -> Result<String, libcommon::SysinspectError> {
    let reject = |msg: String| {
        log::warn!(
            "Datastore minion-auth bootstrap rejected for {} {} from {:?}: {}",
            req.method(),
            req.uri(),
            req.peer_addr(),
            msg
        );
        libcommon::SysinspectError::WebAPIError(msg)
    };

    let minion_id = req
        .headers()
        .get("X-Sysinspect-Minion-Id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| reject("Missing X-Sysinspect-Minion-Id header".to_string()))?;
    let timestamp = req
        .headers()
        .get("X-Sysinspect-Timestamp")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| reject("Missing X-Sysinspect-Timestamp header".to_string()))?;
    let signature_b64 = req
        .headers()
        .get("X-Sysinspect-Signature")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| reject("Missing X-Sysinspect-Signature header".to_string()))?;
    let body_sha256 = req.headers().get("X-Sysinspect-Body-Sha256").and_then(|v| v.to_str().ok()).map(str::trim).unwrap_or("");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|e| reject(format!("Unable to read system time: {e}")))?
        .as_secs();
    let ts = timestamp
        .parse::<u64>()
        .map_err(|_| reject("Invalid X-Sysinspect-Timestamp header".to_string()))?;
    if now.abs_diff(ts) > MINION_AUTH_SKEW_SECS {
        return Err(reject("Expired minion-auth timestamp".to_string()));
    }

    let cfg = {
        let master = master.lock().await;
        master.cfg().await.clone()
    };
    let key_path = cfg.minion_keys_root().join(format!("{minion_id}.rsa.pub"));
    let public_key = match key_from_file(key_path.to_str().unwrap_or_default())
        .map_err(|e| reject(format!("Unable to load minion public key: {e}")))?
    {
        Some(RsaKey::Public(pbk)) => pbk,
        _ => {
            return Err(reject(format!("Unknown minion or invalid public key for {}", minion_id)))
        }
    };
    let signature = STANDARD
        .decode(signature_b64)
        .map_err(|e| reject(format!("Invalid minion-auth signature encoding: {e}")))?;
    let material = minion_auth_material(req.method().as_str(), req.path(), req.query_string(), timestamp, body_sha256);
    let verified = verify_sign(&public_key, material.as_bytes(), signature)
        .map_err(|e| reject(format!("Minion-auth verification failed: {e}")))?;
    if !verified {
        return Err(reject("Invalid minion-auth signature".to_string()));
    }

    Ok(minion_id.to_string())
}

#[utoipa::path(
    post,
    path = "/store/auth/minion",
    tag = "Datastore",
    responses(
        (status = 200, description = "Datastore minion authentication successful", body = StoreMinionAuthResponse),
        (status = 401, description = "Unauthorized", body = StoreErrorResponse)
    )
)]
#[post("/store/auth/minion")]
pub async fn store_minion_auth_handler(req: HttpRequest, master: web::Data<MasterInterfaceType>) -> impl Responder {
    match verify_minion_bootstrap(&req, &master).await {
        Ok(minion_id) => match get_session_store().lock().await.open(format!("minion:{minion_id}")) {
            Ok(token) => HttpResponse::Ok().json(StoreMinionAuthResponse {
                status: "authenticated".to_string(),
                access_token: token,
                token_type: "Bearer".to_string(),
                error: String::new(),
            }),
            Err(err) => HttpResponse::InternalServerError().json(StoreErrorResponse { error: err.to_string() }),
        },
        Err(err) => HttpResponse::Unauthorized().json(StoreErrorResponse { error: err.to_string() }),
    }
}

/// Get a list of all meta files within the datastore.
fn get_meta_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for ent in std::fs::read_dir(root)? {
        let ent = ent?;
        let p = ent.path();
        let ft = ent.file_type()?;
        if ft.is_dir() {
            get_meta_files(&p, out)?;
        } else if ft.is_file() {
            // match *.meta.json
            if p.file_name().and_then(|s| s.to_str()).map(|s| s.ends_with(".meta.json")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/store/{sha256}",
    tag = "Datastore",
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("sha256" = String, Path, description = "SHA256 of the stored object")
    ),
    responses(
        (status = 200, description = "Metadata for object", body = StoreMetaResponse),
        (status = 401, description = "Unauthorized", body = StoreErrorResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Datastore error")
    )
)]
#[get("/store/{sha256:[0-9a-fA-F]{64}}")]
pub async fn store_meta_handler(req: HttpRequest, master: web::Data<MasterInterfaceType>, sha256: web::Path<String>) -> impl Responder {
    if let Err(err) = authorise_request(&req).await {
        return HttpResponse::Unauthorized().json(StoreErrorResponse { error: err.to_string() });
    }
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
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("sha256" = String, Path, description = "SHA256 of the stored object")
    ),
    responses(
        (status = 401, description = "Unauthorized", body = StoreErrorResponse),
        (status = 200, description = "Binary blob"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Datastore error")
    )
)]
#[get("/store/{sha256:[0-9a-fA-F]{64}}/blob")]
pub async fn store_blob_handler(req: HttpRequest, master: web::Data<MasterInterfaceType>, sha256: web::Path<String>) -> ActixResult<NamedFile> {
    authorise_request(&req).await.map_err(unauthorised_store_error)?;
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
    security(
        ("bearer_auth" = [])
    ),
    request_body(
        content = Vec<u8>,
        content_type = "application/octet-stream",
        description = "Raw bytes to store"
    ),
    responses(
        (status = 401, description = "Unauthorized", body = StoreErrorResponse),
        (status = 200, description = "Stored successfully", body = StoreMetaResponse),
        (status = 413, description = "Payload too large"),
        (status = 500, description = "Datastore error")
    )
)]
#[post("/store")]
pub async fn store_upload_handler(req: actix_web::HttpRequest, master: web::Data<MasterInterfaceType>, mut payload: web::Payload) -> impl Responder {
    if let Err(err) = authorise_request(&req).await {
        return HttpResponse::Unauthorized().json(StoreErrorResponse { error: err.to_string() });
    }
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

#[utoipa::path(
    get,
    path = "/store/resolve",
    tag = "Datastore",
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("fname" = String, Query, description = "Full path stored in metadata (meta.fname)")
    ),
    responses(
        (status = 200, description = "Resolved metadata", body = StoreMetaResponse),
        (status = 401, description = "Unauthorized", body = StoreErrorResponse),
        (status = 404, description = "Not found"),
        (status = 500, description = "Error")
    )
)]
#[get("/store/resolve")]
pub async fn store_resolve_handler(req: HttpRequest, master: web::Data<MasterInterfaceType>, q: web::Query<StoreResolveQuery>) -> impl Responder {
    if let Err(err) = authorise_request(&req).await {
        return HttpResponse::Unauthorized().json(StoreErrorResponse { error: err.to_string() });
    }
    let (root, want) = {
        let m = master.lock().await;
        (m.cfg().await.datastore_path(), q.fname.clone())
    };

    let res = task::spawn_blocking(move || -> std::io::Result<Option<DataItemMeta>> {
        let mut metafiles = Vec::<PathBuf>::new();
        get_meta_files(&root, &mut metafiles)?;

        let mut best: Option<DataItemMeta> = None;
        for mp in metafiles {
            let bytes = match std::fs::read(&mp) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let meta: DataItemMeta = match serde_json::from_slice(&bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if meta.fname.as_deref() != Some(want.as_str()) {
                continue;
            }

            best = match best {
                None => Some(meta),
                Some(cur) => {
                    if meta.created_unix >= cur.created_unix {
                        Some(meta)
                    } else {
                        Some(cur)
                    }
                }
            };
        }

        Ok(best)
    })
    .await;

    let meta = match res {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    match meta {
        Some(meta) => HttpResponse::Ok().json(StoreMetaResponse {
            sha256: meta.sha256,
            size_bytes: meta.size_bytes,
            fmode: meta.fmode,
            created_unix: meta.created_unix,
            expires_unix: meta.expires_unix,
            fname: meta.fname,
        }),
        None => HttpResponse::NotFound().finish(),
    }
}

#[utoipa::path(
    get,
    path = "/store/list",
    tag = "Datastore",
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("prefix" = Option<String>, Query, description = "Only return items where meta.fname starts with this prefix"),
        ("limit" = Option<usize>, Query, description = "Max items to return (default 200)")
    ),
    responses(
        (status = 200, description = "List of metadata", body = Vec<StoreMetaResponse>),
        (status = 401, description = "Unauthorized", body = StoreErrorResponse),
        (status = 500, description = "Error")
    )
)]
#[get("/store/list")]
pub async fn store_list_handler(req: HttpRequest, master: web::Data<MasterInterfaceType>, q: web::Query<StoreListQuery>) -> impl Responder {
    if let Err(err) = authorise_request(&req).await {
        return HttpResponse::Unauthorized().json(StoreErrorResponse { error: err.to_string() });
    }
    let (root, prefix, limit) = {
        let m = master.lock().await;
        (m.cfg().await.datastore_path(), q.prefix.clone(), q.limit.unwrap_or(200).min(5000))
    };

    let res = task::spawn_blocking(move || -> std::io::Result<Vec<DataItemMeta>> {
        let mut metafiles = Vec::<PathBuf>::new();
        get_meta_files(&root, &mut metafiles)?;

        let mut out = Vec::<DataItemMeta>::new();
        for mp in metafiles {
            if out.len() >= limit {
                break;
            }
            let bytes = match std::fs::read(&mp) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let meta: DataItemMeta = match serde_json::from_slice(&bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if let Some(ref pfx) = prefix
                && !meta.fname.as_deref().map(|s| s.starts_with(pfx)).unwrap_or(false)
            {
                continue;
            }

            out.push(meta);
        }

        // newest first
        out.sort_by_key(|b| std::cmp::Reverse(b.created_unix));
        if out.len() > limit {
            out.truncate(limit);
        }

        Ok(out)
    })
    .await;

    let metas = match res {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    HttpResponse::Ok().json(
        metas
            .into_iter()
            .map(|meta| StoreMetaResponse {
                sha256: meta.sha256,
                size_bytes: meta.size_bytes,
                fmode: meta.fmode,
                created_unix: meta.created_unix,
                expires_unix: meta.expires_unix,
                fname: meta.fname,
            })
            .collect::<Vec<_>>(),
    )
}
