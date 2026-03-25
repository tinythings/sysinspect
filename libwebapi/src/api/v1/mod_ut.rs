use super::V1;

#[test]
fn docs_are_enabled_reports_constructor_state() {
    assert!(V1::new(false, true).docs_are_enabled());
    assert!(!V1::new(false, false).docs_are_enabled());
}

#[test]
fn openapi_document_uses_production_description_when_not_in_devmode() {
    assert!(
        V1::new(false, true)
            .openapi_document()
            .info
            .description
            .unwrap_or_default()
            .contains("Use HTTPS/TLS for all requests")
    );
}

#[test]
fn openapi_document_uses_development_description_when_in_devmode() {
    assert!(
        V1::new(true, true)
            .openapi_document()
            .info
            .description
            .unwrap_or_default()
            .contains("Development mode is enabled")
    );
}
