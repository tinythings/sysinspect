use crate::api::ApiVersions;
use actix_web::{App, HttpServer, web};
use colored::Colorize;
use libcommon::SysinspectError;
use libdatastore::resources::DataStorage;
use libsysinspect::cfg::mmconf::MasterConfig;
use once_cell::sync::OnceCell;
use rustls::RootCertStore;
use rustls::ServerConfig;
use rustls::server::WebPkiClientVerifier;
use std::{fs::File, io::BufReader, sync::Arc, thread};
use tokio::sync::Mutex;
use x509_parser::prelude::parse_x509_certificate;

pub mod api;
#[cfg(test)]
mod lib_ut;
#[cfg(feature = "pam")]
pub mod pamauth;
pub mod sessions;

#[async_trait::async_trait]
pub trait MasterInterface: Send + Sync {
    async fn cfg(&self) -> &MasterConfig;
    async fn query(&mut self, query: String) -> Result<(), SysinspectError>;
    async fn datastore(&self) -> Arc<Mutex<DataStorage>>;
}

pub type MasterInterfaceType = Arc<Mutex<dyn MasterInterface + Send + Sync + 'static>>;

static RUSTLS_PROVIDER: OnceCell<()> = OnceCell::new();

pub fn ensure_rustls_crypto_provider() -> Result<(), SysinspectError> {
    RUSTLS_PROVIDER.get_or_try_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| SysinspectError::WebAPIError("Failed to install rustls crypto provider".to_string()))?;
        Ok::<(), SysinspectError>(())
    })?;
    Ok(())
}

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

fn advertised_doc_message(bind_addr: &str, bind_port: u32, tls_enabled: bool, doc_enabled: bool) -> String {
    if doc_enabled {
        return format!("Embedded Web API enabled. Swagger UI available at {}", advertised_doc_url(bind_addr, bind_port, tls_enabled));
    }

    "Embedded Web API enabled. API documentation is not enabled.".to_string()
}

fn devmode_doc_warning_message(dev_mode: bool, doc_enabled: bool) -> Option<String> {
    (dev_mode && doc_enabled).then(|| {
        "Embedded Web API is running with api.devmode=true while API documentation is enabled. Swagger UI reflects development authentication behavior and should not be exposed in production.".to_string()
    })
}

fn tls_context_summary(cfg: &MasterConfig) -> String {
    format!(
        "doc={}, client-auth={}, {}",
        if cfg.api_doc_enabled() { "enabled" } else { "disabled" },
        if cfg.api_tls_ca_file().is_some() { "required" } else { "disabled" },
        tls_paths_summary(cfg)
    )
}

/// Returns a user-friendly error message about TLS setup for WebAPI, pointing to the relevant documentation section.
pub(crate) fn tls_setup_err_message() -> String {
    "TLS is not setup for WebAPI. For more information, see Documentation chapter \"Configuration\", section \"api.tls.enabled\".".to_string()
}

/// Returns a user-friendly warning message about using a self-signed TLS certificate for WebAPI, pointing to the relevant configuration option.
pub(crate) fn tls_self_signed_warning_message() -> String {
    format!(
        "Embedded Web API is using a {} TLS certificate because {} is set to {}. Clients must explicitly trust this certificate.",
        "self-signed".bright_red(),
        "api.tls.allow-insecure".bright_yellow(),
        "true".bright_yellow()
    )
}

fn cert_appears_self_signed(cert_der: &[u8]) -> Result<bool, SysinspectError> {
    let (_, cert) = parse_x509_certificate(cert_der)
        .map_err(|err| SysinspectError::ConfigError(format!("Unable to parse Web API TLS certificate for trust checks: {err}")))?;
    Ok(cert.tbs_certificate.subject == cert.tbs_certificate.issuer)
}

/// Loads the TLS server configuration for the Web API from the provided MasterConfig.
/// This includes reading the certificate and private key files, and optionally
/// the CA file for client certificate authentication.
/// Returns a ServerConfig on success, or a SysinspectError with a user-friendly message on failure.
fn load_tls_server_config(cfg: &MasterConfig) -> Result<ServerConfig, SysinspectError> {
    ensure_rustls_crypto_provider()?;

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
    if cert_appears_self_signed(certs[0].as_ref())? && !cfg.api_tls_allow_insecure() {
        return Err(SysinspectError::ConfigError(format!(
            "Web API TLS certificate file {} appears to be self-signed. Set api.tls.allow-insecure=true only when you intentionally want to allow this setup.",
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

    let builder = if let Some(ca_path) = cfg.api_tls_ca_file() {
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
        let mut roots = RootCertStore::empty();
        for ca_cert in ca_certs {
            roots
                .add(ca_cert)
                .map_err(|err| SysinspectError::ConfigError(format!("Unable to use Web API TLS CA file {}: {err}", ca_path.display())))?;
        }

        let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|err| SysinspectError::ConfigError(format!("Invalid Web API TLS CA verifier configuration: {err}")))?;
        ServerConfig::builder().with_client_cert_verifier(verifier)
    } else {
        ServerConfig::builder().with_no_client_auth()
    };

    builder
        .with_single_cert(certs, private_key)
        .map_err(|err| SysinspectError::ConfigError(format!("Invalid Web API TLS certificate/private key pair: {err}")))
}

fn tls_paths_summary(cfg: &MasterConfig) -> String {
    format!(
        "cert={}, key={}, ca={}",
        cfg.api_tls_cert_file().map(|p| p.display().to_string()).unwrap_or_else(|| "<unset>".to_string()),
        cfg.api_tls_key_file().map(|p| p.display().to_string()).unwrap_or_else(|| "<unset>".to_string()),
        cfg.api_tls_ca_file().map(|p| p.display().to_string()).unwrap_or_else(|| "<unset>".to_string())
    )
}

/// Starts the embedded Web API server in a new thread, using the provided MasterConfig and MasterInterface.
pub fn start_embedded_webapi(cfg: MasterConfig, master: MasterInterfaceType) -> Result<(), SysinspectError> {
    if !cfg.api_enabled() {
        log::info!("Embedded Web API disabled.");
        return Ok(());
    }
    ensure_rustls_crypto_provider()?;

    if !cfg.api_tls_enabled() {
        log::error!(
            "{}",
            "TLS is not setup for WebAPI. For more information, see Documentation chapter \"Configuration\", section \"api.tls.enabled\"."
                .replace("\"Configuration\"", &format!("\"{}\"", "Configuration".bright_yellow()))
                .replace("\"api.tls.enabled\"", &format!("\"{}\"", "api.tls.enabled".bright_yellow()))
        );
        return Ok(());
    }

    let tls_config = match load_tls_server_config(&cfg) {
        Ok(tls_config) => tls_config,
        Err(err) => {
            log::error!(
                "{}",
                "TLS is not setup for WebAPI. For more information, see Documentation chapter \"Configuration\", section \"api.tls.enabled\"."
                    .replace("\"Configuration\"", &format!("\"{}\"", "Configuration".bright_yellow()))
                    .replace("\"api.tls.enabled\"", &format!("\"{}\"", "api.tls.enabled".bright_yellow()))
            );
            log::error!("Embedded Web API TLS setup error: {err}");
            log::error!("Embedded Web API TLS context: {}", tls_context_summary(&cfg));
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
        log::info!("{}", advertised_doc_message(&bind_addr, bind_port, true, ccfg.api_doc_enabled()).yellow());
        log::info!("Embedded Web API TLS context: {}", tls_context_summary(&ccfg));
        if let Some(msg) = devmode_doc_warning_message(devmode, ccfg.api_doc_enabled()) {
            log::warn!("{msg}");
        }
        if ccfg.api_tls_allow_insecure() {
            log::warn!("{}", tls_self_signed_warning_message());
        }
        actix_web::rt::System::new().block_on(async move {
            let server = HttpServer::new(move || {
                let mut scope = web::scope("");
                if let Some(ver) = api::get(devmode, ccfg.api_doc_enabled(), version) {
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
