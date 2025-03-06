use ipc::ipc_service_server::{IpcService, IpcServiceServer};
use ipc::{EmptyRequest, QueryRequest, QueryResponse, Record, RecordsResponse};
use std::sync::Arc;
use tokio::{net::UnixListener, sync::Mutex};
use tonic::{Request, Response, Status, transport::Server};

use crate::kvdb::EventsRegistry;

pub mod ipc {
    tonic::include_proto!("ipc");
}

#[derive(Debug)]
pub struct DbIPCService {
    db: Arc<sled::Db>,
    evtreg: Arc<Mutex<EventsRegistry>>,
}

#[tonic::async_trait]
impl IpcService for Arc<DbIPCService> {
    async fn get_records(&self, _request: Request<EmptyRequest>) -> Result<Response<RecordsResponse>, Status> {
        let mut records = Vec::new();

        for tree_name in self.db.tree_names() {
            let tree = self.db.open_tree(&tree_name).map_err(|e| Status::internal(e.to_string()))?;

            for item in tree.iter() {
                if let Ok((key, value)) = item {
                    records.push(Record {
                        key: String::from_utf8_lossy(&key).to_string(),
                        value: value.to_vec(),
                        tree: String::from_utf8_lossy(&tree_name).to_string(),
                    });
                }
            }
        }

        Ok(Response::new(RecordsResponse { records }))
    }

    async fn query(&self, request: Request<QueryRequest>) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        log::info!("Got request: {:#?}", req);
        let tree = self.db.open_tree(&req.tree).map_err(|e| Status::internal(e.to_string()))?;

        tree.insert(req.key.as_bytes(), req.value).map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(QueryResponse { success: true }))
    }
}

impl DbIPCService {
    pub fn new(db_path: &str, evtreg: Arc<Mutex<EventsRegistry>>) -> Result<Self, sled::Error> {
        let db = sled::open(db_path)?;
        Ok(Self { db: Arc::new(db), evtreg })
    }

    /// Run IPC service using Unix socket (path)
    pub async fn run(self: Arc<Self>, socket_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("IPC socket: {socket_path}");
        let _ = std::fs::remove_file(socket_path);
        let uds = UnixListener::bind(socket_path)?;

        let incoming = async_stream::stream! {
            loop {
                let res = uds.accept().await;
                if let Ok((stream, _)) = res {
                    yield Ok::<_, std::io::Error>(stream);
                }
            }
        };

        log::info!("Starting IPC endpoint on {}", socket_path);
        Server::builder().add_service(IpcServiceServer::new(Arc::clone(&self))).serve_with_incoming(incoming).await?;
        log::info!("IPC endpoint is listening on {}", socket_path);

        Ok(())
    }
}
