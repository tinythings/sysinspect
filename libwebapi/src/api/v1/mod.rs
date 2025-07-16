use crate::MasterInterfaceArc;
use actix_web::{HttpResponse, Responder, Scope, post, web};
use serde::Deserialize;
use serde_json::json;

/// API Version 1 implementation
pub struct V1;
impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        scope.service(query_handler).service(health_handler)
    }
}

#[derive(Deserialize)]
pub struct QueryRequest {
    pub query: String,
}
#[post("/v1/query")]
async fn query_handler(master: web::Data<MasterInterfaceArc>, body: web::Json<QueryRequest>) -> impl Responder {
    println!("Query handler called");
    let mut lock = master.lock().await;
    match lock.query(body.query.clone()).await {
        Ok(msg) => HttpResponse::Ok().json(msg),
        Err(err) => HttpResponse::Ok().json(json!({"error": err.to_string()})),
    }
}

#[post("/v1/health")]
pub async fn health_handler(master: web::Data<MasterInterfaceArc>, _body: web::Bytes) -> impl Responder {
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
