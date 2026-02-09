use crate::kvdb::{EventData, EventMinion, EventSession, EventsRegistry};
use crate::{QUERY_CMD_PURGE_ALL, QUERY_CYCLES, QUERY_EVENTS, QUERY_MINIONS};
use ipc::ipc_service_server::{IpcService, IpcServiceServer};
use ipc::{EmptyRequest, QueryRequest, QueryResponse, Record, RecordsResponse};
use libcommon::SysinspectError;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{net::UnixListener, sync::Mutex};
use tonic::{Request, Response, Status, transport::Server};

pub mod ipc {
    tonic::include_proto!("ipc");
}

#[derive(Debug)]
pub struct DbIPCService {
    evtreg: Arc<Mutex<EventsRegistry>>,
    socket_path: String,
}

#[tonic::async_trait]
impl IpcService for Arc<DbIPCService> {
    async fn get_records(&self, _request: Request<EmptyRequest>) -> Result<Response<RecordsResponse>, Status> {
        let mut records = Vec::new();
        log::info!("Getting records");
        for x in self.get_sessions().await.map_err(|e| Status::internal(e.to_string()))? {
            records.push(Record {
                key: x.sid().to_string(),
                value: serde_json::to_vec(&x).map_err(|e| Status::internal(e.to_string()))?,
                tree: "sessions".to_string(),
            });
        }

        Ok(Response::new(RecordsResponse { records }))
    }

    async fn query(&self, request: Request<QueryRequest>) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        let mut records = Vec::new();
        match req.command.as_str() {
            QUERY_CYCLES => {
                for s in self.get_sessions().await.map_err(|e| Status::internal(e.to_string()))? {
                    records.push(Record {
                        key: s.sid().to_string(),
                        value: serde_json::to_vec(&s).map_err(|e| Status::internal(e.to_string()))?,
                        tree: "".to_string(),
                    });
                }
            }

            QUERY_MINIONS => {
                for m in self.get_minions(req.tree.as_str()).await.map_err(|e| Status::internal(e.to_string()))? {
                    records.push(Record {
                        key: m.id().to_string(),
                        value: serde_json::to_vec(&m).map_err(|e| Status::internal(e.to_string()))?,
                        tree: req.tree.clone(),
                    });
                }
            }

            QUERY_EVENTS => {
                for e in self.get_events(&req.tree, &req.key).await.map_err(|e| Status::internal(e.to_string()))? {
                    records.push(Record {
                        key: req.key.clone(),
                        value: serde_json::to_vec(&e).map_err(|e| Status::internal(e.to_string()))?,
                        tree: req.tree.clone(),
                    });
                }
            }

            QUERY_CMD_PURGE_ALL => {
                log::info!("Purging all events");
                self.purge_all().await.map_err(|e| Status::internal(e.to_string()))?;
            }

            _ => log::info!("Got unknown command: {:#?}", req.command),
        };

        Ok(Response::new(QueryResponse { success: true, records }))
    }
}

impl DbIPCService {
    pub fn new(evtreg: Arc<Mutex<EventsRegistry>>, sock: &str) -> Result<Self, sled::Error> {
        Ok(Self { evtreg, socket_path: sock.to_owned() })
    }

    /// Open a session within the event registration context
    pub async fn open_session(&self, model: String, sid: String, ts: String) -> Result<EventSession, SysinspectError> {
        self.evtreg.lock().await.open_session(model, sid, ts)
    }

    /// Ensure minion is recorded within the session
    pub async fn ensure_minion(
        &self, sid: &EventSession, mid: String, traits: HashMap<String, Value>,
    ) -> Result<std::string::String, SysinspectError> {
        self.evtreg.lock().await.ensure_minion(sid, mid, traits)
    }

    /// Add an event for the minion
    pub async fn add_event(&self, sid: &EventSession, mid: EventMinion, payload: HashMap<String, Value>) -> Result<(), SysinspectError> {
        self.evtreg.lock().await.add_event(sid, mid, payload)
    }

    /// Get sessions
    pub async fn get_sessions(&self) -> Result<Vec<EventSession>, SysinspectError> {
        self.evtreg.lock().await.get_sessions()
    }

    pub async fn get_session(&self, sid: &str) -> Result<EventSession, SysinspectError> {
        self.evtreg.lock().await.get_session(sid)
    }

    /// Get last session
    pub async fn get_last_session(&self) -> Result<EventSession, SysinspectError> {
        self.evtreg.lock().await.get_last_session()
    }

    /// Get minions
    pub async fn get_minions(&self, sid: &str) -> Result<Vec<EventMinion>, SysinspectError> {
        self.evtreg.lock().await.get_minions(sid)
    }

    /// Get events
    pub async fn get_events(&self, sid: &str, mid: &str) -> Result<Vec<EventData>, SysinspectError> {
        self.evtreg.lock().await.get_events(sid, mid)
    }

    /// Get all events
    pub async fn purge_all(&self) -> Result<(), SysinspectError> {
        self.evtreg.lock().await.purge_all_data()
    }

    /// Run IPC service using Unix socket (path)
    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("IPC socket: {}", self.socket_path);
        let _ = std::fs::remove_file(&self.socket_path);
        let uds = UnixListener::bind(&self.socket_path)?;

        let incoming = async_stream::stream! {
            loop {
                let res = uds.accept().await;
                if let Ok((stream, _)) = res {
                    yield Ok::<_, std::io::Error>(stream);
                }
            }
        };

        log::info!("Starting IPC endpoint on {}", self.socket_path);
        Server::builder().add_service(IpcServiceServer::new(Arc::clone(&self))).serve_with_incoming(incoming).await?;
        log::info!("IPC endpoint is listening on {}", self.socket_path);

        Ok(())
    }
}
