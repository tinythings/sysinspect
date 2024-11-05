use crate::config::MasterConfig;
use libsysinspect::{
    proto::{self, MinionMessage},
    SysinspectError,
};
use std::{path::Path, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};
use tokio::{fs::OpenOptions, sync::Mutex};

#[derive(Debug)]
pub struct SysMaster {
    cfg: MasterConfig,
    broadcast: broadcast::Sender<Vec<u8>>,
}

impl SysMaster {
    pub fn new(cfg: MasterConfig) -> SysMaster {
        let (tx, _) = broadcast::channel::<Vec<u8>>(100);

        SysMaster { cfg, broadcast: tx }
    }

    /// Open FIFO socket for command-line communication
    fn open_socket(&self, path: &str) -> Result<(), SysinspectError> {
        if !Path::new(path).exists() {
            if unsafe { libc::mkfifo(std::ffi::CString::new(path)?.as_ptr(), 0o600) } != 0 {
                return Err(SysinspectError::ConfigError(format!("{}", std::io::Error::last_os_error())));
            }
            log::info!("Socket opened at {}", path);
        }
        Ok(())
    }

    /// Parse minion request
    fn to_request(&self, data: &str) -> Option<MinionMessage> {
        match serde_json::from_str::<proto::MinionMessage>(data) {
            Ok(request) => {
                return Some(request);
            }
            Err(err) => {
                log::error!("Error parse minion response: {err}");
            }
        }

        None
    }

    /// Start sysmaster
    pub async fn init(&mut self) -> Result<(), SysinspectError> {
        log::info!("Starting master at {}", self.cfg.bind_addr());
        self.open_socket(&self.cfg.socket())?;
        Ok(())
    }

    pub fn cfg(&self) -> MasterConfig {
        self.cfg.to_owned()
    }

    pub fn broadcast(&self) -> broadcast::Sender<Vec<u8>> {
        self.broadcast.clone()
    }

    pub async fn listener(&self) -> Result<TcpListener, SysinspectError> {
        Ok(TcpListener::bind(self.cfg.bind_addr()).await?)
    }

    /// Process incoming minion messages
    pub async fn do_incoming(master: Arc<Mutex<Self>>, mut rx: tokio::sync::mpsc::Receiver<(Vec<u8>, usize)>) {
        log::trace!("Init incoming channel");
        tokio::spawn(async move {
            loop {
                if let Some((msg, client_id)) = rx.recv().await {
                    let msg = String::from_utf8_lossy(&msg).to_string();
                    log::trace!("Minion response: {}: {}", client_id, msg);
                    if let Some(req) = master.lock().await.to_request(&msg) {
                        match req.req_type() {
                            proto::rqtypes::RequestType::Add => {
                                log::info!("Add");
                            }
                            proto::rqtypes::RequestType::Response => {
                                log::info!("Response");
                            }
                            proto::rqtypes::RequestType::Ehlo => {
                                log::info!("Ehlo from {}", req.id());
                            }
                            _ => {
                                log::error!("Minion sends unknown request type");
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    pub async fn do_fifo(master: Arc<Mutex<Self>>) {
        log::trace!("Init local command channel");
        tokio::spawn(async move {
            let bcast = master.lock().await.broadcast();
            let cfg = master.lock().await.cfg();
            loop {
                match OpenOptions::new().read(true).open(cfg.socket()).await {
                    Ok(file) => {
                        let reader = TokioBufReader::new(file);
                        let mut lines = reader.lines();

                        loop {
                            select! {
                                line = lines.next_line() => {
                                    match line {
                                        Ok(Some(message)) => {
                                            log::info!("Broadcasting FIFO message to clients: {}", message);
                                            let _ = bcast.send(message.into_bytes());
                                        }
                                        Ok(None) => break, // End of file, re-open the FIFO
                                        Err(e) => {
                                            log::error!("Error reading from FIFO: {}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to open FIFO: {}", e);
                        sleep(Duration::from_secs(1)).await; // Retry after a sec
                    }
                }
            }
        });
    }

    pub async fn do_outgoing(
        master: Arc<Mutex<Self>>, tx: tokio::sync::mpsc::Sender<(Vec<u8>, usize)>,
    ) -> Result<(), SysinspectError> {
        log::trace!("Init outgoing channel");
        let listener = master.lock().await.listener().await?;
        tokio::spawn(async move {
            let bcast = master.lock().await.broadcast();
            let mut client_id_counter: usize = 0;
            loop {
                if let Ok((socket, _)) = listener.accept().await {
                    client_id_counter += 1;
                    let current_client_id = client_id_counter;
                    let mut rx = bcast.subscribe();
                    let client_tx = tx.clone();

                    let (reader, writer) = socket.into_split();

                    // Task to send messages to the client
                    tokio::spawn(async move {
                        let mut writer = writer;
                        log::info!("Minion {} connected. Ready to send messages.", current_client_id);
                        loop {
                            if let Ok(msg) = rx.recv().await {
                                log::info!("Sending message to client {}: {:?}", current_client_id, msg);
                                if writer.write_all(&(msg.len() as u32).to_be_bytes()).await.is_err()
                                    || writer.write_all(&msg).await.is_err()
                                    || writer.flush().await.is_err()
                                {
                                    break;
                                }
                            }
                        }
                    });

                    // Task to read messages from the client
                    tokio::spawn(async move {
                        let mut reader = TokioBufReader::new(reader);
                        loop {
                            let mut len_buf = [0u8; 4];
                            if reader.read_exact(&mut len_buf).await.is_err() {
                                return;
                            }

                            let msg_len = u32::from_be_bytes(len_buf) as usize;
                            let mut msg = vec![0u8; msg_len];
                            if reader.read_exact(&mut msg).await.is_err() {
                                return;
                            }

                            if client_tx.send((msg, current_client_id)).await.is_err() {
                                break;
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }
}

pub(crate) async fn master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let master = Arc::new(Mutex::new(SysMaster::new(cfg)));
    {
        let mut m = master.lock().await;
        m.init().await?;
    }

    let (client_tx, client_rx) = mpsc::channel::<(Vec<u8>, usize)>(100);

    // Task to read from the FIFO and broadcast messages to clients
    SysMaster::do_fifo(Arc::clone(&master)).await;

    // Handle incoming messages from minions
    SysMaster::do_incoming(Arc::clone(&master), client_rx).await;

    // Accept connections and spawn tasks for each client
    SysMaster::do_outgoing(Arc::clone(&master), client_tx).await?;

    // Listen for shutdown signal and cancel tasks
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Received shutdown signal.");
    std::process::exit(0);
}
