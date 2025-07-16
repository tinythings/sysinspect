use actix_web::{HttpResponse, Responder, Scope, post, web};

/// API Version 1 implementation
pub struct V1;
impl super::ApiVersion for V1 {
    fn load(&self, scope: Scope) -> Scope {
        scope.service(query_handler).service(health_handler)
    }
}

#[post("/api/v1/query")]
async fn query_handler(_body: web::Bytes) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"result": "Done"}))
}

#[post("/api/v1/health")]
async fn health_handler(_body: web::Bytes) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "healthy"}))
}
