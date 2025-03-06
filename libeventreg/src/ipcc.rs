use hyper_util::rt::tokio::TokioIo;
use ipc::ipc_service_client::IpcServiceClient;
use ipc::{EmptyRequest, QueryRequest};
use std::error::Error;
use std::sync::Arc;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

pub mod ipc {
    tonic::include_proto!("ipc");
}

impl QueryRequest {
    pub fn new(k: &str, v: &str, t: &str, c: &str) -> Self {
        QueryRequest { key: k.to_string(), value: v.as_bytes().to_vec(), tree: t.to_string(), command: c.to_string() }
    }
}

#[derive(Debug)]
pub struct DbIPCClient {
    client: IpcServiceClient<Channel>,
    uds_path: Arc<String>, // ✅ Store as owned String inside an Arc
}

impl DbIPCClient {
    pub async fn xnew(uds_path: impl Into<String>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let uds_path = Arc::new(uds_path.into()); // ✅ Convert &str into owned String

        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector({
                let uds_path = Arc::clone(&uds_path); // ✅ Clone Arc so it moves safely
                service_fn(move |_| {
                    let uds_path = Arc::clone(&uds_path);
                    async move {
                        let stream = UnixStream::connect(&*uds_path).await?;
                        Ok::<_, std::io::Error>(TokioIo::new(stream))
                    }
                })
            })
            .await?;

        Ok(Self { client: IpcServiceClient::new(channel), uds_path })
    }

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

        Ok(Self { client: IpcServiceClient::new(channel), uds_path })
    }

    /// **Insert a record into the database**
    pub async fn insert(&mut self, i: i32) -> Result<(), Box<dyn Error>> {
        self.client.query(QueryRequest::new(&format!("foo-{}", i), "bar", "default", "")).await?;
        Ok(())
    }

    /// **Fetch all records from the IPC service**
    pub async fn fetch_records(&mut self) -> Result<(), Box<dyn Error>> {
        let response = self.client.get_records(EmptyRequest {}).await?;
        let records = response.into_inner().records;

        println!("Received {} records:", records.len());
        for record in records {
            println!("Key: {}, Tree: {}, Value ({} bytes)", record.key, record.tree, record.value.len());
        }

        Ok(())
    }

    /// **Run test operations (insert & fetch)**
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Inserting sample records...");

        for x in 0..100 {
            self.insert(x).await?;
        }

        println!("Fetching records...");
        self.fetch_records().await?;

        Ok(())
    }
}
