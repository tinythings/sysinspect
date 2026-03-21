use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use colored::Colorize;
use libcommon::SysinspectError;
use libdatastore::resources::DataStorage;
use libsysinspect::cfg::mmconf::MasterConfig;
use rustls::ServerConfig;
use std::{fs::File, io::BufReader, sync::Arc, thread};
use tokio::sync::Mutex;

pub mod api;
#[cfg(test)]
mod lib_ut;
pub mod pamauth;
pub mod sessions;

#[async_trait::async_trait]
pub trait MasterInterface: Send + Sync {
    async fn cfg(&self) -> &MasterConfig;
    async fn query(&mut self, query: String) -> Result<(), SysinspectError>;
    async fn datastore(&self) -> Arc<Mutex<DataStorage>>;
}

pub type MasterInterfaceType = Arc<Mutex<dyn MasterInterface + Send + Sync + 'static>>;

/// Determines the advertised API host for the Web API based on the bind address.
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

/// Constructs the advertised documentation URL for the Web API based on the bind address, port, and TLS configuration.
pub(crate) fn advertised_doc_url(bind_addr: &str, bind_port: u32, tls_enabled: bool) -> String {
    let scheme = if tls_enabled { "https" } else { "http" };
    format!("{scheme}://{}:{bind_port}/doc/", advertised_api_host(bind_addr))
}

/// Returns a user-friendly error message about TLS setup for WebAPI, pointing to the relevant documentation section.
pub(crate) fn tls_setup_err_message() -> String {
    format!(
        "TLS is not setup for WebAPI. For more information, see Documentation chapter \"{}\", section \"{}\".",
        "Configuration".bright_yellow(),
        "api.tls.enabled".bright_yellow()
    )
}

/// Loads the TLS server configuration for the Web API from the provided MasterConfig.
/// This includes reading the certificate and private key files, and optionally
/// the CA file if client certificate authentication is configured.
/// Returns a ServerConfig on success, or a SysinspectError with a user-friendly message on failure.
fn load_tls_server_config(cfg: &MasterConfig) -> Result<ServerConfig, SysinspectError> {
    let cert_path = cfg
        .api_tls_cert_file()
        .ok_or_else(|| SysinspectError::ConfigError("Web API TLS is enabled, but api.tls.cert-file is not configured".to_string()))?;
    let key_path = cfg
        .api_tls_key_file()
        .ok_or_else(|| SysinspectError::ConfigError("Web API TLS is enabled, but api.tls.key-file is not configured".to_string()))?;

    let mut cert_reader = BufReader::new(
        File::open(&cert_path)
            .map_err(|err| SysinspectError::ConfigError(format!("Unable to open Web API TLS certificate file {}: {err}", cert_path.display())))?,
    );
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| SysinspectError::ConfigError(format!("Unable to read Web API TLS certificate file {}: {err}", cert_path.display())))?;
    if certs.is_empty() {
        return Err(SysinspectError::ConfigError(format!(
            "Web API TLS certificate file {} does not contain any PEM certificates",
            cert_path.display()
        )));
    }

    let mut key_reader = BufReader::new(
        File::open(&key_path)
            .map_err(|err| SysinspectError::ConfigError(format!("Unable to open Web API TLS private key file {}: {err}", key_path.display())))?,
    );
    let private_key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|err| SysinspectError::ConfigError(format!("Unable to read Web API TLS private key file {}: {err}", key_path.display())))?
        .ok_or_else(|| {
            SysinspectError::ConfigError(format!("Web API TLS private key file {} does not contain a supported PEM private key", key_path.display()))
        })?;

    if let Some(ca_path) = cfg.api_tls_ca_file() {
        let mut ca_reader = BufReader::new(
            File::open(&ca_path)
                .map_err(|err| SysinspectError::ConfigError(format!("Unable to open Web API TLS CA file {}: {err}", ca_path.display())))?,
        );
        let ca_certs = rustls_pemfile::certs(&mut ca_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| SysinspectError::ConfigError(format!("Unable to read Web API TLS CA file {}: {err}", ca_path.display())))?;
        if ca_certs.is_empty() {
            return Err(SysinspectError::ConfigError(format!("Web API TLS CA file {} does not contain any PEM certificates", ca_path.display())));
        }
    }

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)
        .map_err(|err| SysinspectError::ConfigError(format!("Invalid Web API TLS certificate/private key pair: {err}")))
}

/// Starts the embedded Web API server in a new thread, using the provided MasterConfig and MasterInterface.
pub fn start_embedded_webapi(cfg: MasterConfig, master: MasterInterfaceType) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Embedded Web API disabled.");
        return Ok(());
    }

    if !cfg.api_tls_enabled() {
        log::error!("{}", tls_setup_err_message());
        return Ok(());
    }

    let tls_config = match load_tls_server_config(&cfg) {
        Ok(tls_config) => tls_config,
        Err(err) => {
            log::error!("{}", tls_setup_err_message());
            log::error!("Embedded Web API TLS setup error: {err}");
            return Ok(());
        }
    };

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

        log::info!("Starting embedded Web API inside sysmaster at {} over {}", listen_addr.bright_yellow(), "HTTPS/TLS");
        log::info!("Embedded Web API enabled. Swagger UI available at {}", advertised_doc_url(&bind_addr, bind_port, true).bright_yellow());
        if ccfg.api_tls_allow_insecure() {
            log::warn!("Web API TLS allow-insecure mode is enabled for clients.");
        }

        actix_web::rt::System::new().block_on(async move {
            let server = HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(devmode, version) {
                    scope = ver.load(scope);
                }
                App::new().app_data(web::Data::new(cmaster.clone())).service(scope)
            });

            let server = server.bind_rustls_0_23((bind_addr.as_str(), bind_port as u16), tls_config).map_err(SysinspectError::from)?;

            server.run().await.map_err(SysinspectError::from)
        })
    });

    Ok(())
}
