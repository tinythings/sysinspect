use crate::MasterInterfaceArc;
use actix_web::{HttpResponse, Responder, Scope, post, web};

/// API Version 1 implementation
pub struct V1;
impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        scope.service(query_handler).service(health_handler)
    }
}

#[post("/v1/query")]
async fn query_handler(_body: web::Bytes) -> impl Responder {
    log::info!("Query handler called");
    HttpResponse::Ok().json(serde_json::json!({"result": "Done"}))
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
