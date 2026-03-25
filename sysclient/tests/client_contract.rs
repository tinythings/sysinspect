use actix_web::{App, HttpServer, web};
use async_trait::async_trait;
use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use libsysinspect::cfg::mmconf::MasterConfig;
use libwebapi::{MasterInterface, MasterInterfaceType, api::{self, ApiVersions}};
use std::{fs, path::Path, sync::Arc};
use sysinspect_client::{ModelNameResponse, QueryResponse, SysClient, SysClientConfiguration};
use tokio::{sync::Mutex, task::JoinHandle, time::{Duration, sleep}};

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

fn write_cfg(root: &Path) -> MasterConfig {
    let cfg_path = root.join("sysinspect.conf");
    fs::write(
        &cfg_path,
        "config:\n  master:\n    fileserver.models: [cm, net]\n    api.bind.ip: 127.0.0.1\n    api.bind.port: 4202\n    api.devmode: true\n",
    )
    .unwrap();
    MasterConfig::new(cfg_path).unwrap()
}

async fn spawn_http_server() -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<std::io::Result<()>>) {
    let root = tempfile::tempdir().unwrap();
    let cfg = write_cfg(root.path());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let datastore = Arc::new(Mutex::new(DataStorage::new(DataStorageConfig::new(), root.path().join("datastore")).unwrap()));
    let master: MasterInterfaceType = Arc::new(Mutex::new(TestMaster { cfg, queries: Arc::clone(&queries), datastore }));
    let server = HttpServer::new(move || {
        let scope = api::get(true, true, ApiVersions::V1).unwrap().load(web::scope(""));
        App::new().app_data(web::Data::new(master.clone())).service(scope)
    })
    .bind(("127.0.0.1", 0))
    .unwrap();
    let addr = server.addrs()[0];
    let handle = tokio::spawn(server.run());
    sleep(Duration::from_millis(100)).await;

    (format!("http://{}", addr), queries, handle)
}

#[tokio::test]
async fn client_authenticates_and_executes_plain_json_query() {
    let (base, queries, handle) = spawn_http_server().await;
    let mut client = SysClient::new(SysClientConfiguration { master_url: base, });

    let token = client.authenticate("dev", "dev").await.unwrap();
    let response: QueryResponse = client.query("cm/file-ops", "*", "", "", serde_json::json!({"reason":"test"})).await.unwrap();

    assert_eq!(token, "dev-token");
    assert_eq!(response.status, "success");
    assert_eq!(queries.lock().await.as_slice(), ["cm/file-ops;*;;;reason:test"]);
    handle.abort();
}

#[tokio::test]
async fn client_lists_models_using_bearer_auth() {
    let (base, _, handle) = spawn_http_server().await;
    let mut client = SysClient::new(SysClientConfiguration { master_url: base });
    client.authenticate("dev", "dev").await.unwrap();

    let models: ModelNameResponse = client.models().await.unwrap();

    assert_eq!(models.models, vec!["cm".to_string(), "net".to_string()]);
    handle.abort();
}
