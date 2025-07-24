pub use crate::api::v1::system::health_handler;
use crate::api::v1::{
    minions::{QueryError, QueryPayloadRequest, QueryRequest, QueryResponse, query_handler},
    pkeys::{MasterKeyError, MasterKeyResponse, PubKeyError, PubKeyRequest, PubKeyResponse, masterkey_handler, pushkey_handler},
    system::{AuthInnerRequest, AuthRequest, AuthResponse, HealthInfo, HealthResponse, authenticate_handler},
};
use actix_web::Scope;
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

/// API Version 1 implementation
pub struct V1;
impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        scope
            .service(SwaggerUi::new("/api-doc/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .service(query_handler)
            .service(health_handler)
            .service(authenticate_handler)
            .service(pushkey_handler)
            .service(masterkey_handler)
    }
}

#[derive(OpenApi)]
#[openapi(paths(crate::api::v1::minions::query_handler,
                crate::api::v1::system::health_handler,
                crate::api::v1::system::authenticate_handler,
                crate::api::v1::pkeys::pushkey_handler,
                crate::api::v1::pkeys::masterkey_handler),
          components(schemas(QueryRequest, QueryResponse, QueryError, QueryPayloadRequest,
                             PubKeyRequest, PubKeyResponse, PubKeyError, MasterKeyResponse, MasterKeyError,
                             HealthInfo, HealthResponse, AuthRequest, AuthResponse, AuthInnerRequest)),
info(title = "SysInspect API", version = API_VERSION, description = "SysInspect Web API for interacting with the master interface."))]
pub struct ApiDoc;
