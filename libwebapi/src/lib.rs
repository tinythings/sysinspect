use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig};
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

pub mod api;
pub mod pamauth;
pub mod sessions;

#[async_trait::async_trait]
pub trait MasterInterface: Send + Sync {
    /// Returns a reference to the master configuration.
    async fn cfg(&self) -> &MasterConfig;

    /// Query minions
    async fn query(&mut self, query: String) -> Result<(), SysinspectError>;
}

pub type MasterInterfaceArc = Arc<Mutex<dyn MasterInterface + Send + Sync + 'static>>;
pub fn start_webapi(cfg: MasterConfig, master: MasterInterfaceArc) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Web API is disabled in the configuration.");
        return Ok(());
    }

    let ccfg = cfg.clone();
    let cmaster = master.clone();

    thread::spawn(move || {
        let version = match ccfg.api_version() {
            1 => ApiVersions::V1,
            _ => ApiVersions::V1,
        };

        actix_web::rt::System::new().block_on(async move {
            HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(version) {
                    scope = ver.load(scope);
                }
                App::new().app_data(web::Data::new(cmaster.clone())).service(scope)
            })
            .bind((ccfg.api_bind_addr(), ccfg.api_bind_port() as u16))
            .map_err(SysinspectError::from)?
            .run()
            .await
            .map_err(SysinspectError::from)
        })
    });

    Ok(())
}
