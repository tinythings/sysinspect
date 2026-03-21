use actix_web::{App, HttpServer, web};
use async_trait::async_trait;
use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use libsysinspect::cfg::mmconf::MasterConfig;
use libwebapi::{MasterInterface, MasterInterfaceType, api::{self, ApiVersions}};
use reqwest::Certificate;
use rustls::ServerConfig;
use std::{fs, io::BufReader, path::Path, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle, time::{Duration, sleep}};

const CERT_PEM: &str = include_str!("data/sysmaster-dev.crt");
const KEY_PEM: &str = include_str!("data/sysmaster-dev.key");

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

fn write_cfg(root: &Path, devmode: bool) -> MasterConfig {
    let cfg_path = root.join("sysinspect.conf");
    fs::write(
        &cfg_path,
        format!(
            "config:\n  master:\n    fileserver.models: [cm, net]\n    api.bind.ip: 127.0.0.1\n    api.bind.port: 4202\n    api.devmode: {}\n",
            if devmode { "true" } else { "false" }
        ),
    )
    .unwrap();
    MasterConfig::new(cfg_path).unwrap()
}

fn tls_config() -> ServerConfig {
    let mut cert_reader = BufReader::new(CERT_PEM.as_bytes());
    let certs = rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>().unwrap();
    let mut key_reader = BufReader::new(KEY_PEM.as_bytes());
    let key = rustls_pemfile::private_key(&mut key_reader).unwrap().unwrap();

    ServerConfig::builder().with_no_client_auth().with_single_cert(certs, key).unwrap()
}

async fn spawn_https_server(devmode: bool) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<std::io::Result<()>>) {
    let root = tempfile::tempdir().unwrap();
    let cfg = write_cfg(root.path(), devmode);
    let queries = Arc::new(Mutex::new(Vec::new()));
    let datastore = Arc::new(Mutex::new(DataStorage::new(DataStorageConfig::new(), root.path().join("datastore")).unwrap()));
    let master: MasterInterfaceType = Arc::new(Mutex::new(TestMaster { cfg, queries: Arc::clone(&queries), datastore }));

    let server = HttpServer::new(move || {
        let scope = api::get(devmode, ApiVersions::V1).unwrap().load(web::scope(""));
        App::new().app_data(web::Data::new(master.clone())).service(scope)
    })
    .bind_rustls_0_23(("127.0.0.1", 0), tls_config())
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

#[tokio::test]
async fn https_server_rejects_default_certificate_validation_for_self_signed_cert() {
    let (base, _, handle) = spawn_https_server(true).await;

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
async fn https_auth_and_query_use_plain_json_and_bearer_token() {
    let (base, queries, handle) = spawn_https_server(true).await;
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
    let (base, _, handle) = spawn_https_server(true).await;
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
    let (base, _, handle) = spawn_https_server(true).await;
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
