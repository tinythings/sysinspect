use actix_web::{App, HttpServer, web};
use async_trait::async_trait;
use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use libsysinspect::cfg::mmconf::MasterConfig;
use libwebapi::{MasterInterface, MasterInterfaceType, api::{self, ApiVersions}};
use reqwest::{Certificate, Identity};
use rustls::ServerConfig;
use rustls::RootCertStore;
use rustls::server::WebPkiClientVerifier;
use std::{fs, io::BufReader, path::Path, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle, time::{Duration, sleep}};

const CERT_PEM: &str = include_str!("data/sysmaster-dev.crt");
const KEY_PEM: &str = include_str!("data/sysmaster-dev.key");
const MTLS_CA_CERT_PEM: &str = include_str!("data/webapi-test-ca.crt");
const MTLS_SERVER_CERT_PEM: &str = include_str!("data/webapi-test-server.crt");
const MTLS_SERVER_KEY_PEM: &str = include_str!("data/webapi-test-server.key");
const MTLS_CLIENT_CERT_PEM: &str = include_str!("data/webapi-test-client.crt");
const MTLS_CLIENT_KEY_PEM: &str = include_str!("data/webapi-test-client.key");

struct TestMaster {
    cfg: MasterConfig,
    queries: Arc<Mutex<Vec<String>>>,
    datastore: Arc<Mutex<DataStorage>>,
}

#[async_trait]
impl MasterInterface for TestMaster {
    async fn cfg(&self) -> &MasterConfig {
        &self.cfg
    }

    async fn query(&mut self, query: String) -> Result<(), libcommon::SysinspectError> {
        self.queries.lock().await.push(query);
        Ok(())
    }

    async fn datastore(&self) -> Arc<Mutex<DataStorage>> {
        Arc::clone(&self.datastore)
    }
}

fn write_cfg(root: &Path, devmode: bool, doc_enabled: bool) -> MasterConfig {
    let cfg_path = root.join("sysinspect.conf");
    fs::write(
        &cfg_path,
        format!(
            "config:\n  master:\n    fileserver.models: [cm, net]\n    api.bind.ip: 127.0.0.1\n    api.bind.port: 4202\n    api.devmode: {}\n    api.doc: {}\n",
            if devmode { "true" } else { "false" },
            if doc_enabled { "true" } else { "false" }
        ),
    )
    .unwrap();
    MasterConfig::new(cfg_path).unwrap()
}

fn tls_config(require_client_auth: bool) -> ServerConfig {
    let (server_cert_pem, server_key_pem, ca_cert_pem) = if require_client_auth {
        (MTLS_SERVER_CERT_PEM, MTLS_SERVER_KEY_PEM, Some(MTLS_CA_CERT_PEM))
    } else {
        (CERT_PEM, KEY_PEM, None)
    };

    let mut cert_reader = BufReader::new(server_cert_pem.as_bytes());
    let certs = rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>().unwrap();
    let mut key_reader = BufReader::new(server_key_pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut key_reader).unwrap().unwrap();

    let builder = if let Some(ca_cert_pem) = ca_cert_pem {
        let mut ca_reader = BufReader::new(ca_cert_pem.as_bytes());
        let ca_certs = rustls_pemfile::certs(&mut ca_reader).collect::<Result<Vec<_>, _>>().unwrap();
        let mut roots = RootCertStore::empty();
        for ca_cert in ca_certs {
            roots.add(ca_cert).unwrap();
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(roots)).build().unwrap();
        ServerConfig::builder().with_client_cert_verifier(verifier)
    } else {
        ServerConfig::builder().with_no_client_auth()
    };

    builder.with_single_cert(certs, key).unwrap()
}

async fn spawn_https_server(devmode: bool, doc_enabled: bool, require_client_auth: bool) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<std::io::Result<()>>) {
    let root = tempfile::tempdir().unwrap();
    let cfg = write_cfg(root.path(), devmode, doc_enabled);
    let queries = Arc::new(Mutex::new(Vec::new()));
    let datastore = Arc::new(Mutex::new(DataStorage::new(DataStorageConfig::new(), root.path().join("datastore")).unwrap()));
    let master: MasterInterfaceType = Arc::new(Mutex::new(TestMaster { cfg, queries: Arc::clone(&queries), datastore }));

    let server = HttpServer::new(move || {
        let scope = api::get(devmode, doc_enabled, ApiVersions::V1).unwrap().load(web::scope(""));
        App::new().app_data(web::Data::new(master.clone())).service(scope)
    })
    .bind_rustls_0_23(("127.0.0.1", 0), tls_config(require_client_auth))
    .unwrap();
    let addr = server.addrs()[0];
    let handle = tokio::spawn(server.run());
    sleep(Duration::from_millis(100)).await;

    (format!("https://{}", addr), queries, handle)
}

fn trusted_client() -> reqwest::Client {
    reqwest::Client::builder()
        .add_root_certificate(Certificate::from_pem(CERT_PEM.as_bytes()).unwrap())
        .build()
        .unwrap()
}

fn trusted_mtls_client() -> reqwest::Client {
    reqwest::Client::builder()
        .add_root_certificate(Certificate::from_pem(MTLS_CA_CERT_PEM.as_bytes()).unwrap())
        .build()
        .unwrap()
}

fn trusted_client_with_identity() -> reqwest::Client {
    reqwest::Client::builder()
        .add_root_certificate(Certificate::from_pem(MTLS_CA_CERT_PEM.as_bytes()).unwrap())
        .identity(Identity::from_pkcs8_pem(MTLS_CLIENT_CERT_PEM.as_bytes(), MTLS_CLIENT_KEY_PEM.as_bytes()).unwrap())
        .build()
        .unwrap()
}

#[tokio::test]
async fn https_server_rejects_default_certificate_validation_for_self_signed_cert() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;

    let err = reqwest::Client::new()
        .post(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap_err()
        .to_string();

    handle.abort();
    assert!(!err.is_empty());
}

#[tokio::test]
async fn https_swagger_ui_rejects_default_certificate_validation_for_self_signed_cert() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let err = reqwest::Client::new().get(format!("{base}/doc/")).send().await.unwrap_err().to_string();

    handle.abort();
    assert!(!err.is_empty());
}

#[tokio::test]
async fn https_openapi_json_rejects_default_certificate_validation_for_self_signed_cert() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let err = reqwest::Client::new().get(format!("{base}/api-doc/openapi.json")).send().await.unwrap_err().to_string();

    handle.abort();
    assert!(!err.is_empty());
}

#[tokio::test]
async fn https_auth_and_query_use_plain_json_and_bearer_token() {
    let (base, queries, handle) = spawn_https_server(true, true, false).await;
    let client = trusted_client();

    let auth = client
        .post(format!("{base}/api/v1/authenticate"))
        .json(&serde_json::json!({"username":"dev","password":"dev"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let token = auth["access_token"].as_str().unwrap().to_string();

    let query = client
        .post(format!("{base}/api/v1/query"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "model":"cm/file-ops",
            "query":"*",
            "traits":"",
            "mid":"",
            "context":{"reason":"test"}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(query["status"], "success");
    assert_eq!(queries.lock().await.as_slice(), ["cm/file-ops;*;;;reason:test"]);
    handle.abort();
}

#[tokio::test]
async fn https_query_rejects_missing_bearer_token() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let client = trusted_client();

    let response = client
        .post(format!("{base}/api/v1/query"))
        .json(&serde_json::json!({
            "model":"cm/file-ops",
            "query":"*",
            "traits":"",
            "mid":"",
            "context":{}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    handle.abort();
}

#[tokio::test]
async fn https_model_names_returns_plain_json_list() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let client = trusted_client();
    let auth = client
        .post(format!("{base}/api/v1/authenticate"))
        .json(&serde_json::json!({"username":"dev","password":"dev"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let response = client
        .get(format!("{base}/api/v1/model/names"))
        .bearer_auth(auth["access_token"].as_str().unwrap())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["models"], serde_json::json!(["cm", "net"]));
    handle.abort();
}

#[tokio::test]
async fn https_model_names_rejects_missing_bearer_token_with_json_error() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let client = trusted_client();

    let response = client
        .get(format!("{base}/api/v1/model/names"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["error"], "Error Web API: Missing Authorization header");
    handle.abort();
}

#[tokio::test]
async fn https_store_list_rejects_missing_bearer_token_with_json_error() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let client = trusted_client();

    let response = client
        .get(format!("{base}/store/list"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["error"], "Error Web API: Missing Authorization header");
    handle.abort();
}

#[tokio::test]
async fn https_server_rejects_requests_without_required_client_certificate() {
    let (base, _, handle) = spawn_https_server(true, true, true).await;
    let err = trusted_mtls_client()
        .post(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap_err()
        .to_string();

    assert!(!err.is_empty());
    handle.abort();
}

#[tokio::test]
async fn https_server_accepts_requests_with_trusted_client_certificate() {
    let (base, _, handle) = spawn_https_server(true, true, true).await;
    let response = trusted_client_with_identity()
        .post(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["status"], "healthy");
    handle.abort();
}

#[tokio::test]
async fn https_server_rejects_swagger_without_required_client_certificate() {
    let (base, _, handle) = spawn_https_server(true, true, true).await;
    let err = trusted_mtls_client().get(format!("{base}/doc/")).send().await.unwrap_err().to_string();

    assert!(!err.is_empty());
    handle.abort();
}

#[tokio::test]
async fn https_server_accepts_swagger_with_trusted_client_certificate() {
    let (base, _, handle) = spawn_https_server(true, true, true).await;
    let response = trusted_client_with_identity().get(format!("{base}/doc/")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    handle.abort();
}

#[tokio::test]
async fn https_swagger_ui_is_available_when_api_doc_is_enabled() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let response = trusted_client().get(format!("{base}/doc/")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    handle.abort();
}

#[tokio::test]
async fn https_swagger_ui_is_not_available_when_api_doc_is_disabled() {
    let (base, _, handle) = spawn_https_server(true, false, false).await;
    let response = trusted_client().get(format!("{base}/doc/")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    handle.abort();
}

#[tokio::test]
async fn https_openapi_json_is_available_when_api_doc_is_enabled() {
    let (base, _, handle) = spawn_https_server(false, true, false).await;
    let response = trusted_client().get(format!("{base}/api-doc/openapi.json")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers().get(reqwest::header::CONTENT_TYPE).unwrap(), "application/json");
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["components"]["securitySchemes"]["bearer_auth"]["type"], "http");
    assert_eq!(body["components"]["securitySchemes"]["bearer_auth"]["scheme"], "bearer");
    assert!(body["info"]["description"].as_str().unwrap().contains("Use HTTPS/TLS for all requests"));
    assert!(body["info"]["description"].as_str().unwrap().contains("POST /api/v1/authenticate"));
    assert!(body["info"]["description"].as_str().unwrap().contains("api.doc"));
    handle.abort();
}

#[tokio::test]
async fn https_openapi_json_is_not_available_when_api_doc_is_disabled() {
    let (base, _, handle) = spawn_https_server(true, false, false).await;
    let response = trusted_client().get(format!("{base}/api-doc/openapi.json")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    handle.abort();
}

#[tokio::test]
async fn https_openapi_json_in_dev_mode_mentions_development_auth_behavior() {
    let (base, _, handle) = spawn_https_server(true, true, false).await;
    let body = trusted_client()
        .get(format!("{base}/api-doc/openapi.json"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert!(body["info"]["description"].as_str().unwrap().contains("Development mode is enabled"));
    assert!(body["info"]["description"].as_str().unwrap().contains("development token"));
    handle.abort();
}

#[tokio::test]
async fn https_swagger_asset_is_not_available_when_api_doc_is_disabled() {
    let (base, _, handle) = spawn_https_server(true, false, false).await;
    let response = trusted_client().get(format!("{base}/doc/swagger-ui.css")).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    handle.abort();
}

#[tokio::test]
async fn https_auth_endpoint_still_works_when_api_doc_is_disabled() {
    let (base, _, handle) = spawn_https_server(true, false, false).await;
    let response = trusted_client()
        .post(format!("{base}/api/v1/authenticate"))
        .json(&serde_json::json!({"username":"dev","password":"dev"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    handle.abort();
}
