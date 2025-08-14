pub use crate::api::v1::system::health_handler;
use crate::api::v1::{
    minions::{QueryError, QueryPayloadRequest, QueryRequest, QueryResponse, query_handler, query_handler_dev},
    pkeys::{MasterKeyError, MasterKeyResponse, PubKeyError, PubKeyRequest, PubKeyResponse, masterkey_handler, pushkey_handler},
    system::{AuthInnerRequest, AuthRequest, AuthResponse, HealthInfo, HealthResponse, authenticate_handler},
};
use actix_web::Scope;
use colored::Colorize;
use once_cell::sync::OnceCell;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod minions;
pub mod pkeys;
pub mod system;

const API_VERSION: &str = "0.1.0";

/// API Tags
pub static TAG_MINIONS: &str = "Minions";
pub static TAG_SYSTEM: &str = "System";
pub static TAG_RSAKEYS: &str = "RSA Keys";

static SWAGGER_DEVMODE: OnceCell<std::sync::Mutex<bool>> = OnceCell::new();

/// Get the Swagger UI development mode status.
fn get_is_devmode() -> bool {
    if let Some(mode) = SWAGGER_DEVMODE.get() {
        return *mode.lock().unwrap();
    }

    false
}

/// API Version 1 implementation
pub struct V1 {
    dev_mode: bool,
    swagger_port: u16,
}

impl V1 {
    pub fn new(dev_mode: bool, swagger_port: u16) -> Self {
        V1 { dev_mode, swagger_port }
    }
}

impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        let mut scope = scope
            // Available services
            .service(query_handler)
            .service(health_handler)
            .service(authenticate_handler)
            .service(pushkey_handler)
            .service(masterkey_handler);

        if self.dev_mode {
            scope = scope.service(SwaggerUi::new("/doc/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())).service(query_handler_dev);
            let mode = SWAGGER_DEVMODE.get_or_init(|| std::sync::Mutex::new(false));
            let mut mode = mode.lock().unwrap();
            if !*mode {
                *mode = self.dev_mode;
                log::info!(
                    "{} In development mode {} is enabled at http://{}:{}/doc/",
                    "WARNING:".bright_red().bold(),
                    "API Swagger UI".bright_yellow(),
                    "<THIS_HOST>",
                    self.swagger_port
                );
            }
        }

        scope
    }
}

#[derive(OpenApi)]
#[openapi(paths(crate::api::v1::minions::query_handler,
                crate::api::v1::minions::query_handler_dev,
                crate::api::v1::system::health_handler,
                crate::api::v1::system::authenticate_handler,
                crate::api::v1::pkeys::pushkey_handler,
                crate::api::v1::pkeys::masterkey_handler),
          components(schemas(QueryRequest, QueryResponse, QueryError, QueryPayloadRequest,
                             PubKeyRequest, PubKeyResponse, PubKeyError, MasterKeyResponse, MasterKeyError,
                             HealthInfo, HealthResponse, AuthRequest, AuthResponse, AuthInnerRequest)),
info(title = "SysInspect API", version = API_VERSION, description = "SysInspect Web API for interacting with the master interface."))]
pub struct ApiDoc;
