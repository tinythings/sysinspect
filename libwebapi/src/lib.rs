use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use colored::Colorize;
use libcommon::SysinspectError;
use libdatastore::resources::DataStorage;
use libsysinspect::cfg::mmconf::MasterConfig;
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

pub mod api;
pub mod keystore;
pub mod pamauth;
pub mod sessions;

#[async_trait::async_trait]
pub trait MasterInterface: Send + Sync {
    async fn cfg(&self) -> &MasterConfig;
    async fn query(&mut self, query: String) -> Result<(), SysinspectError>;
    async fn datastore(&self) -> Arc<Mutex<DataStorage>>;
}

pub type MasterInterfaceType = Arc<Mutex<dyn MasterInterface + Send + Sync + 'static>>;

pub fn start_webapi(cfg: MasterConfig, master: MasterInterfaceType) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::warn!("Web API is {} in the configuration.", "disabled".bright_yellow().bold());
        return Ok(());
    }

    let ccfg = cfg.clone();
    let cmaster = master.clone();

    thread::spawn(move || {
        let devmode = ccfg.api_devmode();
        let swagger_port = ccfg.api_bind_port();
        let version = match ccfg.api_version() {
            1 => ApiVersions::V1,
            _ => ApiVersions::V1,
        };

        if devmode {
            log::info!("{} *** {} ***", "WARNING:".bright_red().bold(), "Web API is running in development mode".red());
        } else {
            log::info!(
                "{} is running in {}. Swagger UI is {}.",
                "Web API".yellow(),
                "production mode".bright_green(),
                "disabled".bright_white().bold()
            );
        }

        actix_web::rt::System::new().block_on(async move {
            HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(devmode, swagger_port as u16, version) {
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
