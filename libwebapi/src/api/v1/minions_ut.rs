use super::authorise_request;
use crate::sessions::get_session_store;
use actix_web::{http::header::AUTHORIZATION, test::TestRequest};

#[tokio::test]
async fn authorise_request_accepts_lowercase_bearer_scheme() {
    get_session_store().lock().await.open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap();

    assert_eq!(
        authorise_request(&TestRequest::default().insert_header((AUTHORIZATION, "bearer dev-token")).to_http_request()).await.unwrap(),
        "dev"
    );
}

#[tokio::test]
async fn authorise_request_accepts_mixed_case_bearer_scheme() {
    get_session_store().lock().await.open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap();

    assert_eq!(
        authorise_request(&TestRequest::default().insert_header((AUTHORIZATION, "BeArEr dev-token")).to_http_request()).await.unwrap(),
        "dev"
    );
}

#[tokio::test]
async fn authorise_request_rejects_non_bearer_scheme() {
    assert!(authorise_request(&TestRequest::default().insert_header((AUTHORIZATION, "Basic dev-token")).to_http_request()).await.is_err());
}
