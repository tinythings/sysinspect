use super::{advertised_doc_url, tls_setup_err_message};

#[test]
fn advertised_doc_url_uses_http_without_tls() {
    assert_eq!(advertised_doc_url("127.0.0.1", 4202, false), "http://127.0.0.1:4202/doc/");
}

#[test]
fn advertised_doc_url_uses_https_with_tls() {
    assert_eq!(advertised_doc_url("127.0.0.1", 4202, true), "https://127.0.0.1:4202/doc/");
}

#[test]
fn tls_not_setup_error_points_to_real_docs_section() {
    assert_eq!(
        tls_setup_err_message(),
        "TLS is not setup for WebAPI. For more information, see Documentation chapter \"Configuration\", section \"api.tls.enabled\"."
    );
}
