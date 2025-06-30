use hyper_util::rt::tokio::TokioIo;
use ipc::ipc_service_client::IpcServiceClient;
use ipc::{EmptyRequest, QueryRequest, QueryResponse};
use std::error::Error;
use std::sync::Arc;
use tokio::net::UnixStream;
use tonic::Response;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

pub mod ipc {
    tonic::include_proto!("ipc");
}

impl QueryRequest {
    /// Query params:
    /// - `k`: Key to look for
    /// - `v`: Value value
    /// - `t`: Tree where to look for the key
    /// - `c`: Command (arbitrary)
    pub fn new(k: &str, v: &str, t: &str, c: &str) -> Self {
        QueryRequest { key: k.to_string(), value: v.as_bytes().to_vec(), tree: t.to_string(), command: c.to_string() }
    }
}

#[derive(Debug)]
pub struct DbIPCClient {
    client: IpcServiceClient<Channel>,
}

impl DbIPCClient {
    pub async fn new(uds_path: impl Into<String>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let uds_path = Arc::new(uds_path.into());

        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector({
                let uds_path = Arc::clone(&uds_path);
                service_fn(move |_| {
                    let uds_path = Arc::clone(&uds_path);
                    async move {
                        let stream = UnixStream::connect(&*uds_path).await?;
                        Ok::<_, std::io::Error>(TokioIo::new(stream))
                    }
                })
            })
            .await?;

        Ok(Self { client: IpcServiceClient::new(channel) })
    }

    /// **Insert a record into the database**
    pub async fn insert(&mut self, i: i32) -> Result<(), Box<dyn Error>> {
        self.client.query(QueryRequest::new(&format!("foo-{i}"), "bar", "default", "")).await?;
        Ok(())
    }

    /// Query the IPC service
    pub async fn query(&mut self, k: &str, v: &str, t: &str, c: &str) -> Result<Response<QueryResponse>, Box<dyn Error>> {
        let r = QueryRequest::new(k, v, t, c);
        Ok(self.client.query(r).await?)
    }

    /// Fetch all records from the IPC service
    pub async fn fetch_records(&mut self) -> Result<(), Box<dyn Error>> {
        let response = self.client.get_records(EmptyRequest {}).await?;
        let records = response.into_inner().records;

        log::info!("Received {} records:", records.len());
        for record in records {
            log::info!("Key: {}, Tree: {}, Value: {}", record.key, record.tree, String::from_utf8(record.value).unwrap());
        }

        Ok(())
    }

    /// Run test operations (insert & fetch)
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Inserting sample records...");

        println!("Fetching records...");
        self.fetch_records().await?;

        Ok(())
    }
}
