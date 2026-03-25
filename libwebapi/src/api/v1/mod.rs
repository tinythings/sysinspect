pub use crate::api::v1::system::health_handler;
use crate::api::v1::{
    minions::{QueryError, QueryRequest, QueryResponse, query_handler},
    model::{ModelNameResponse, model_descr_handler, model_names_handler},
    store::{
        StoreListQuery, StoreMetaResponse, StoreResolveQuery, store_blob_handler, store_list_handler, store_meta_handler, store_resolve_handler,
        store_upload_handler,
    },
    system::{AuthRequest, AuthResponse, HealthInfo, HealthResponse, authenticate_handler},
};
use actix_web::Scope;
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

pub mod minions;
pub mod model;
pub mod store;
pub mod system;

const API_VERSION: &str = "0.1.1";

/// API Tags
pub static TAG_MINIONS: &str = "Minions";
pub static TAG_SYSTEM: &str = "System";
pub static TAG_MODELS: &str = "Models";

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme("bearer_auth", SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()));
        }
    }
}

/// API Version 1 implementation
pub struct V1 {
    dev_mode: bool,
    doc_enabled: bool,
}

impl V1 {
    pub fn new(dev_mode: bool, doc_enabled: bool) -> Self {
        V1 { dev_mode, doc_enabled }
    }

    fn api_scope(&self, scope: Scope) -> Scope {
        scope
            .service(query_handler)
            .service(health_handler)
            .service(authenticate_handler)
            .service(model_names_handler)
            .service(model_descr_handler)
            .service(store_resolve_handler)
            .service(store_list_handler)
            .service(store_meta_handler)
            .service(store_blob_handler)
            .service(store_upload_handler)
    }

    fn doc_service(&self) -> SwaggerUi {
        if self.dev_mode {
            SwaggerUi::new("/doc/{_:.*}").url("/api-doc/openapi.json", ApiDocDev::openapi())
        } else {
            SwaggerUi::new("/doc/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi())
        }
    }
}

impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        if self.doc_enabled {
            return self.api_scope(scope).service(self.doc_service());
        }

        self.api_scope(scope)
    }
}

#[derive(OpenApi)]
#[openapi(paths(
    crate::api::v1::minions::query_handler,
    crate::api::v1::system::health_handler,
    crate::api::v1::system::authenticate_handler,
    crate::api::v1::model::model_names_handler,
    crate::api::v1::model::model_descr_handler,
    crate::api::v1::store::store_meta_handler,
    crate::api::v1::store::store_blob_handler,
    crate::api::v1::store::store_upload_handler,
    crate::api::v1::store::store_resolve_handler,
    crate::api::v1::store::store_list_handler,
),
          components(schemas(QueryRequest, QueryResponse, QueryError,
                             HealthInfo, HealthResponse, AuthRequest, AuthResponse,
                             ModelNameResponse, StoreMetaResponse, StoreResolveQuery, StoreListQuery)),
modifiers(&SecurityAddon),
info(title = "SysInspect API", version = API_VERSION, description = "SysInspect Web API for interacting with the master interface."))]
pub struct ApiDoc;

#[derive(OpenApi)]
#[openapi(paths(
    crate::api::v1::minions::query_handler,
    crate::api::v1::system::health_handler,
    crate::api::v1::system::authenticate_handler,
    crate::api::v1::model::model_names_handler,
    crate::api::v1::model::model_descr_handler,
    crate::api::v1::store::store_meta_handler,
    crate::api::v1::store::store_blob_handler,
    crate::api::v1::store::store_upload_handler,
    crate::api::v1::store::store_resolve_handler,
    crate::api::v1::store::store_list_handler,
),
          components(schemas(QueryRequest, QueryResponse, QueryError,
                             HealthInfo, HealthResponse, AuthRequest, AuthResponse,
                             ModelNameResponse, StoreMetaResponse, StoreResolveQuery, StoreListQuery)),
modifiers(&SecurityAddon),
info(title = "SysInspect API", version = API_VERSION, description = "SysInspect Web API for interacting with the master interface."))]
pub struct ApiDocDev;
