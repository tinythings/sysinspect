use super::authorize_request;
use crate::sessions::get_session_store;
use actix_web::{http::header::AUTHORIZATION, test::TestRequest};

#[test]
fn authorize_request_accepts_lowercase_bearer_scheme() {
    get_session_store().lock().unwrap().open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap();

    assert_eq!(
        authorize_request(&TestRequest::default().insert_header((AUTHORIZATION, "bearer dev-token")).to_http_request()).unwrap(),
        "dev"
    );
}

#[test]
fn authorize_request_accepts_mixed_case_bearer_scheme() {
    get_session_store().lock().unwrap().open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap();

    assert_eq!(
        authorize_request(&TestRequest::default().insert_header((AUTHORIZATION, "BeArEr dev-token")).to_http_request()).unwrap(),
        "dev"
    );
}

#[test]
fn authorize_request_rejects_non_bearer_scheme() {
    assert!(authorize_request(&TestRequest::default().insert_header((AUTHORIZATION, "Basic dev-token")).to_http_request()).is_err());
}
