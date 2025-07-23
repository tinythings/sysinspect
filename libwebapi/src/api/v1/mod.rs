pub use crate::api::v1::system::health_handler;
use crate::{
    MasterInterfaceArc,
    api::v1::{
        pkeys::{masterkey_handler, pushkey_handler},
        system::authenticate_handler,
    },
    sessions::get_session_store,
};
use actix_web::{HttpResponse, Responder, Scope, post, web};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

pub mod pkeys;
pub mod system;

const API_VERSION: &str = "0.1.0";

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
#[openapi(paths(query_handler,
                crate::api::v1::system::health_handler,
                crate::api::v1::system::authenticate_handler,
                crate::api::v1::pkeys::pushkey_handler,
                crate::api::v1::pkeys::masterkey_handler),
          components(schemas(QueryRequest)), info(title = "SysInspect API",
version = API_VERSION, description = "SysInspect Web API for interacting with the master interface."))]
pub struct ApiDoc;

#[derive(Deserialize, ToSchema)]
pub struct QueryRequest {
    pub sid: String,
    pub model: String,
    pub query: String,
    pub traits: String,
    pub mid: String,
    pub context: HashMap<String, String>,
}

impl QueryRequest {
    pub fn to_query(&self) -> String {
        format!(
            "{};{};{};{};{}",
            self.model,
            self.query,
            self.traits,
            self.mid,
            self.context.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",")
        )
    }
}
#[utoipa::path(
    post,
    path = "/api/v1/query",
    request_body = QueryRequest,
    responses(
        (status = 200, description = "Success", body = String),
        (status = 400, description = "Bad Request")
    )
)]
#[post("/api/v1/query")]
async fn query_handler(master: web::Data<MasterInterfaceArc>, body: web::Json<QueryRequest>) -> impl Responder {
    let mut master = master.lock().await;
    let cfg = master.cfg().await;
    if !cfg.api_devmode() && get_session_store().lock().unwrap().uid(&body.sid).is_none() {
        return HttpResponse::BadRequest().json(json!({"error": "Invalid session ID"}));
    }

    match master.query(body.to_query()).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Query executed successfully",
        })),
        Err(err) => HttpResponse::Ok().json(json!({"error": err.to_string()})),
    }
}
