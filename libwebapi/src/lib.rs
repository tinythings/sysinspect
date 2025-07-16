use std::thread;

use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig};

pub mod api;

/// Start the web API server with the specified API version.
pub async fn _start_webapi(cfg: MasterConfig) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Web API is disabled in the configuration.");
        return Ok(());
    }

    log::info!("Starting web API server at {}:{}", cfg.api_bind_addr(), cfg.api_bind_port());

    let version = match cfg.api_version() {
        1 => ApiVersions::V1,
        _ => ApiVersions::V1, // Default to V1 if version is not recognized
    };

    HttpServer::new(move || {
        let mut scope = web::scope("");
        if let Some(ver) = api::get(version) {
            scope = ver.load(scope);
        }
        App::new().service(scope)
    })
    .bind((cfg.api_bind_addr(), cfg.api_bind_port() as u16))
    .map_err(SysinspectError::from)?
    .run()
    .await
    .map_err(SysinspectError::from)
}

pub fn start_webapi(cfg: MasterConfig) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Web API is disabled in the configuration.");
        return Ok(());
    }

    log::info!("Starting web API server at {}:{}", cfg.api_bind_addr(), cfg.api_bind_port());

    let cfg_clone = cfg.clone();
    thread::spawn(move || {
        let version = match cfg_clone.api_version() {
            1 => ApiVersions::V1,
            _ => ApiVersions::V1,
        };

        actix_web::rt::System::new().block_on(async move {
            HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(version) {
                    scope = ver.load(scope);
                }
                App::new().service(scope)
            })
            .bind((cfg_clone.api_bind_addr(), cfg_clone.api_bind_port() as u16))
            .map_err(SysinspectError::from)?
            .run()
            .await
            .map_err(SysinspectError::from)
        })
    });

    Ok(())
}
