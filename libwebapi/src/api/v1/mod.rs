use crate::MasterInterfaceArc;
use actix_web::{HttpResponse, Responder, Scope, post, web};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

const API_VERSION: &str = "0.1.0";

/// API Version 1 implementation
pub struct V1;
impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        scope
            .service(SwaggerUi::new("/api-doc/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .service(query_handler)
            .service(health_handler)
    }
}

#[derive(OpenApi)]
#[openapi(paths(query_handler, health_handler), components(schemas(QueryRequest)), info(title = "SysInspect API", version = API_VERSION))]
pub struct ApiDoc;

#[derive(Deserialize, ToSchema)]
pub struct QueryRequest {
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
            self.context.iter().map(|(k, v)| format!("{}:{}", k, v)).collect::<Vec<_>>().join(",")
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
    let mut lock = master.lock().await;
    match lock.query(body.to_query()).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Query executed successfully",
        })),
        Err(err) => HttpResponse::Ok().json(json!({"error": err.to_string()})),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Health status", body = String)
    )
)]
#[post("/api/v1/health")]
pub async fn health_handler(master: web::Data<MasterInterfaceArc>, _body: ()) -> impl Responder {
    let lock = master.lock().await;
    let cfg = lock.cfg().await;

    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "info": {
            "telemetry.enabled": cfg.telemetry_enabled(),
            "scheduler.tasks": cfg.scheduler().len(),
            "api.version": cfg.api_version(),
        }
    }))
}
