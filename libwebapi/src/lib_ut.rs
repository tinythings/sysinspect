use super::{advertised_doc_message, advertised_doc_url, load_tls_server_config, tls_context_summary, tls_paths_summary, tls_self_signed_warning_message, tls_setup_err_message};
use libsysinspect::cfg::mmconf::MasterConfig;
use std::{fs, path::Path, path::PathBuf};

const CERT_PEM: &str = include_str!("../tests/data/sysmaster-dev.crt");
const KEY_PEM: &str = include_str!("../tests/data/sysmaster-dev.key");

fn write_cfg(root: &Path, extra: &str) -> MasterConfig {
    let cfg_path = root.join("sysinspect.conf");
    fs::write(
        &cfg_path,
        format!(
            "config:\n  master:\n    fileserver.models: []\n    api.bind.ip: 127.0.0.1\n    api.bind.port: 4202\n{extra}"
        ),
    )
    .unwrap();
    MasterConfig::new(cfg_path).unwrap()
}

fn write_tls_fixture(root: &Path) -> (PathBuf, PathBuf) {
    let cert = root.join("sysmaster-dev.crt");
    let key = root.join("sysmaster-dev.key");
    fs::write(&cert, CERT_PEM).unwrap();
    fs::write(&key, KEY_PEM).unwrap();
    (cert, key)
}

#[test]
fn advertised_doc_url_uses_http_without_tls() {
    assert_eq!(advertised_doc_url("127.0.0.1", 4202, false), "http://127.0.0.1:4202/doc/");
}

#[test]
fn advertised_doc_url_uses_https_with_tls() {
    assert_eq!(advertised_doc_url("127.0.0.1", 4202, true), "https://127.0.0.1:4202/doc/");
}

#[test]
fn advertised_doc_message_is_present_when_docs_are_enabled() {
    assert_eq!(
        advertised_doc_message("127.0.0.1", 4202, true, true),
        "Embedded Web API enabled. Swagger UI available at https://127.0.0.1:4202/doc/".to_string()
    );
}

#[test]
fn advertised_doc_message_reports_when_docs_are_disabled() {
    assert_eq!(
        advertised_doc_message("127.0.0.1", 4202, true, false),
        "Embedded Web API enabled. API documentation is not enabled.".to_string()
    );
}

#[test]
fn tls_context_summary_reports_doc_and_client_auth_state() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.doc: false\n    api.tls.cert-file: {}\n    api.tls.key-file: {}\n    api.tls.ca-file: {}\n",
            cert.display(),
            key.display(),
            cert.display()
        ),
    );

    let summary = tls_context_summary(&cfg);
    assert!(summary.contains("doc=disabled"));
    assert!(summary.contains("client-auth=required"));
    assert!(summary.contains(&cert.display().to_string()));
    assert!(summary.contains(&key.display().to_string()));
}

#[test]
fn tls_not_setup_error_points_to_real_docs_section() {
    assert_eq!(
        tls_setup_err_message(),
        "TLS is not setup for WebAPI. For more information, see Documentation chapter \"Configuration\", section \"api.tls.enabled\"."
    );
}

#[test]
fn load_tls_server_config_accepts_valid_certificate_pair() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.tls.enabled: true\n    api.tls.cert-file: {}\n    api.tls.key-file: {}\n    api.tls.allow-insecure: true\n",
            cert.display(),
            key.display()
        ),
    );

    assert!(load_tls_server_config(&cfg).is_ok());
}

#[test]
fn load_tls_server_config_accepts_valid_ca_bundle_for_client_auth() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.tls.enabled: true\n    api.tls.cert-file: {}\n    api.tls.key-file: {}\n    api.tls.ca-file: {}\n    api.tls.allow-insecure: true\n",
            cert.display(),
            key.display(),
            cert.display()
        ),
    );

    assert!(load_tls_server_config(&cfg).is_ok());
}

#[test]
fn load_tls_server_config_rejects_self_signed_certificate_without_allow_insecure() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.tls.enabled: true\n    api.tls.cert-file: {}\n    api.tls.key-file: {}\n",
            cert.display(),
            key.display()
        ),
    );

    let err = load_tls_server_config(&cfg).unwrap_err().to_string();
    assert!(err.contains("self-signed"));
    assert!(err.contains("api.tls.allow-insecure"));
}

#[test]
fn load_tls_server_config_rejects_missing_private_key() {
    let root = tempfile::tempdir().unwrap();
    let (cert, _) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!("    api.tls.enabled: true\n    api.tls.cert-file: {}\n", cert.display()),
    );

    let err = load_tls_server_config(&cfg).unwrap_err().to_string();
    assert!(err.contains("api.tls.key-file"));
}

#[test]
fn load_tls_server_config_rejects_invalid_ca_bundle() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let ca = root.path().join("invalid-ca.pem");
    fs::write(&ca, "not a pem").unwrap();
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.tls.enabled: true\n    api.tls.cert-file: {}\n    api.tls.key-file: {}\n    api.tls.ca-file: {}\n    api.tls.allow-insecure: true\n",
            cert.display(),
            key.display(),
            ca.display()
        ),
    );

    let err = load_tls_server_config(&cfg).unwrap_err().to_string();
    assert!(err.contains("CA file"));
}

#[test]
fn tls_paths_summary_reports_configured_locations() {
    let root = tempfile::tempdir().unwrap();
    let (cert, key) = write_tls_fixture(root.path());
    let cfg = write_cfg(
        root.path(),
        &format!(
            "    api.tls.cert-file: {}\n    api.tls.key-file: {}\n    api.tls.ca-file: {}\n",
            cert.display(),
            key.display(),
            cert.display()
        ),
    );

    let summary = tls_paths_summary(&cfg);
    assert!(summary.contains(&cert.display().to_string()));
    assert!(summary.contains(&key.display().to_string()));
}

#[test]
fn tls_self_signed_warning_is_operator_facing() {
    let msg = tls_self_signed_warning_message();
    assert!(msg.contains("self-signed"));
    assert!(msg.contains("api.tls.allow-insecure"));
    assert!(msg.contains("explicitly trust"));
}
