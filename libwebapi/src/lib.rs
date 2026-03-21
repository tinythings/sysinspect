use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use colored::Colorize;
use libcommon::SysinspectError;
use libdatastore::resources::DataStorage;
use libsysinspect::cfg::mmconf::MasterConfig;
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

pub mod api;
pub mod pamauth;
pub mod sessions;

#[async_trait::async_trait]
pub trait MasterInterface: Send + Sync {
    async fn cfg(&self) -> &MasterConfig;
    async fn query(&mut self, query: String) -> Result<(), SysinspectError>;
    async fn datastore(&self) -> Arc<Mutex<DataStorage>>;
}

pub type MasterInterfaceType = Arc<Mutex<dyn MasterInterface + Send + Sync + 'static>>;

fn advertised_api_host(bind_addr: &str) -> String {
    match bind_addr {
        "0.0.0.0" | "::" | "[::]" => hostname::get()
            .ok()
            .and_then(|value| {
                let host = value.to_string_lossy().trim().to_string();
                if host.is_empty() { None } else { Some(host) }
            })
            .unwrap_or_else(|| "localhost".to_string()),
        _ => bind_addr.to_string(),
    }
}

fn advertised_doc_url(bind_addr: &str, bind_port: u32) -> String {
    format!("http://{}:{bind_port}/doc/", advertised_api_host(bind_addr))
}

pub fn start_embedded_webapi(cfg: MasterConfig, master: MasterInterfaceType) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Embedded Web API disabled.");
        return Ok(());
    }

    let ccfg = cfg.clone();
    let cmaster = master.clone();

    thread::spawn(move || {
        let devmode = ccfg.api_devmode();
        let bind_addr = ccfg.api_bind_addr();
        let bind_port = ccfg.api_bind_port();
        let listen_addr = format!("{}:{}", bind_addr, bind_port);
        let version = match ccfg.api_version() {
            1 => ApiVersions::V1,
            _ => ApiVersions::V1,
        };

        log::info!("Starting embedded Web API inside sysmaster at {}", listen_addr.bright_yellow());
        log::info!("Embedded Web API enabled. Swagger UI available at {}", advertised_doc_url(&bind_addr, bind_port));

        actix_web::rt::System::new().block_on(async move {
            HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(devmode, version) {
                    scope = ver.load(scope);
                }
                App::new().app_data(web::Data::new(cmaster.clone())).service(scope)
            })
            .bind((bind_addr.as_str(), bind_port as u16))
            .map_err(SysinspectError::from)?
            .run()
            .await
            .map_err(SysinspectError::from)
        })
    });

    Ok(())
}
