use crate::{
    dataserv::fls,
    registry::{
        mkb::MinionsKeyRegistry,
        mreg::MinionRegistry,
        session::{self, SessionKeeper},
    },
};
use libsysinspect::{
    cfg::mmconf::MasterConfig,
    mdescr::mspec::MODEL_FILE_EXT,
    proto::{
        self, errcodes::ProtoErrorCode, payload::ModStatePayload, rqtypes::RequestType, MasterMessage, MinionMessage,
        MinionTarget, ProtoConversion,
    },
    util::iofs::scan_files_sha256,
    SysinspectError,
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};
use tokio::{fs::OpenOptions, sync::Mutex};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader},
    time,
};

#[derive(Debug)]
pub struct SysMaster {
    cfg: MasterConfig,
    broadcast: broadcast::Sender<Vec<u8>>,
    mkr: MinionsKeyRegistry,
    mreg: MinionRegistry,
    to_drop: HashSet<String>,
    session: session::SessionKeeper,
}

impl SysMaster {
    pub fn new(cfg: MasterConfig) -> Result<SysMaster, SysinspectError> {
        let (tx, _) = broadcast::channel::<Vec<u8>>(100);
        let mkr = MinionsKeyRegistry::new(cfg.keyman_root())?;
        let mreg = MinionRegistry::new(cfg.minion_registry_root())?;
        Ok(SysMaster { cfg, broadcast: tx, mkr, to_drop: HashSet::default(), session: SessionKeeper::new(30), mreg })
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

    /// Get Minion key registry
    fn mkr(&mut self) -> &mut MinionsKeyRegistry {
        &mut self.mkr
    }

    /// Construct a Command message to the minion
    fn msg_query(&mut self, payload: &str) -> Option<MasterMessage> {
        let query = payload.split(";").map(|s| s.to_string()).collect::<Vec<String>>();

        if let [scheme, query, traits] = query.as_slice() {
            let mut tgt = MinionTarget::default();
            tgt.set_scheme(scheme);
            tgt.set_traits_query(traits);
            for hostname in query.split(",") {
                tgt.add_hostname(hostname);
            }

            // Collect downloadable model(s) files
            let mut out: HashMap<String, String> = HashMap::default();
            for em in self.cfg.fileserver_models() {
                for (n, cs) in scan_files_sha256(self.cfg.fileserver_mdl_root(false).join(em), Some(MODEL_FILE_EXT)) {
                    out.insert(
                        format!("/{}/{em}/{n}", self.cfg.fileserver_mdl_root(false).file_name().unwrap().to_str().unwrap()),
                        cs,
                    );
                }
            }

            let mut msg = MasterMessage::new(
                RequestType::Command,
                json!(ModStatePayload::new("12345".to_string())
                    .set_uri(scheme.to_string())
                    .add_files(out)
                    .set_models_root(self.cfg.fileserver_mdl_root(true).to_str().unwrap_or_default())), // TODO: SID part
            );
            msg.set_target(tgt);
            msg.set_retcode(ProtoErrorCode::Success);

            return Some(msg);
        }

        None
    }

    /// Request minion to sync its traits
    fn msg_request_traits(&mut self, mid: String, sid: String) -> MasterMessage {
        let mut m = MasterMessage::new(RequestType::Traits, json!(sid));
        let tgt = MinionTarget::new(&mid, &sid);
        m.set_target(tgt);
        m.set_retcode(ProtoErrorCode::Success);

        m
    }

    /// Already connected
    fn msg_already_connected(&mut self, mid: String, sid: String) -> MasterMessage {
        let mut m = MasterMessage::new(RequestType::Command, json!(sid));
        m.set_target(MinionTarget::new(&mid, &sid));
        m.set_retcode(ProtoErrorCode::AlreadyConnected);

        m
    }

    /// Bounce message
    fn msg_not_registered(&mut self, mid: String) -> MasterMessage {
        let mut m =
            MasterMessage::new(RequestType::AgentUnknown, json!(self.mkr().get_master_key_pem().clone().unwrap().to_string()));
        m.set_target(MinionTarget::new(&mid, ""));
        m.set_retcode(ProtoErrorCode::Success);

        m
    }

    /// Accept registration
    fn msg_registered(&self, mid: String, msg: &str) -> MasterMessage {
        let mut m = MasterMessage::new(RequestType::Reconnect, json!(msg)); // XXX: Should it be already encrypted?
        m.set_target(MinionTarget::new(&mid, ""));
        m.set_retcode(ProtoErrorCode::Success);

        m
    }

    /// Process incoming minion messages
    #[allow(clippy::while_let_loop)]
    pub async fn do_incoming(master: Arc<Mutex<Self>>, mut rx: tokio::sync::mpsc::Receiver<(Vec<u8>, String)>) {
        log::trace!("Init incoming channel");
        let bcast = master.lock().await.broadcast();
        tokio::spawn(async move {
            loop {
                if let Some((msg, minion_addr)) = rx.recv().await {
                    let msg = String::from_utf8_lossy(&msg).to_string();
                    log::trace!("Minion response: {}: {}", minion_addr, msg);
                    if let Some(req) = master.lock().await.to_request(&msg) {
                        match req.req_type() {
                            RequestType::Add => {
                                let c_master = Arc::clone(&master);
                                let c_bcast = bcast.clone();
                                let c_mid = req.id().to_string();
                                tokio::spawn(async move {
                                    log::info!("Minion \"{}\" requested registration", minion_addr);
                                    let mut guard = c_master.lock().await;
                                    let resp_msg: &str;
                                    if !guard.mkr().is_registered(&c_mid) {
                                        if let Err(err) = guard.mkr().add_mn_key(&c_mid, &minion_addr, req.payload()) {
                                            log::error!("Unable to add minion RSA key: {err}");
                                        }
                                        guard.to_drop.insert(minion_addr.to_owned());
                                        resp_msg = "Minion registration has been accepted";
                                        log::info!("Registered a minion at {minion_addr} ({})", c_mid);
                                    } else {
                                        resp_msg = "Minion already registered";
                                        log::warn!("Minion {minion_addr} ({}) is already registered", c_mid);
                                    }
                                    _ = c_bcast.send(guard.msg_registered(req.id().to_string(), resp_msg).sendable().unwrap());
                                });
                            }
                            RequestType::Response => {
                                log::info!("Response");
                            }
                            RequestType::Ehlo => {
                                log::info!("EHLO from {}", req.id());

                                let c_master = Arc::clone(&master);
                                let c_bcast = bcast.clone();
                                let c_id = req.id().to_string();
                                let c_payload = req.payload().to_string();
                                tokio::spawn(async move {
                                    let mut guard = c_master.lock().await;
                                    if !guard.mkr().is_registered(&c_id) {
                                        log::info!("Minion at {minion_addr} ({}) is not registered", req.id());
                                        guard.to_drop.insert(minion_addr);
                                        _ = c_bcast.send(guard.msg_not_registered(req.id().to_string()).sendable().unwrap());
                                    } else if guard.session.exists(&c_id) {
                                        log::info!("Minion at {minion_addr} ({}) is already connected", req.id());
                                        guard.to_drop.insert(minion_addr);
                                        _ = c_bcast.send(
                                            guard.msg_already_connected(req.id().to_string(), c_payload).sendable().unwrap(),
                                        );
                                    } else {
                                        log::info!("{} connected successfully", c_id);
                                        guard.session.ping(&c_id, &c_payload);
                                        _ = c_bcast
                                            .send(guard.msg_request_traits(req.id().to_string(), c_payload).sendable().unwrap());
                                        log::info!("Syncing traits with minion at {}", c_id);
                                    }
                                });
                            }

                            RequestType::Pong => {
                                let c_master = Arc::clone(&master);
                                let c_id = req.id().to_string();
                                let c_payload = req.payload().to_string();
                                tokio::spawn(async move {
                                    let mut guard = c_master.lock().await;
                                    guard.session.ping(&c_id, &c_payload);
                                    let uptime = guard.session.uptime(req.id()).unwrap_or_default();
                                    log::debug!(
                                        "Update last contacted for {} (alive for {:.2} min)",
                                        req.id().to_string(),
                                        uptime as f64 / 60.0
                                    );
                                });
                            }

                            RequestType::Traits => {
                                log::debug!("Syncing traits from {}", req.id());
                                let c_master = Arc::clone(&master);
                                let c_id = req.id().to_string();
                                let c_payload = req.payload().to_string();
                                tokio::spawn(async move {
                                    let mut guard = c_master.lock().await;
                                    guard.on_traits(c_id, c_payload).await;
                                });
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

    pub async fn on_traits(&mut self, mid: String, payload: String) {
        let traits = serde_json::from_str::<HashMap<String, serde_json::Value>>(&payload).unwrap_or_default();
        if !traits.is_empty() {
            if let Err(err) = self.mreg.refresh(&mid, traits) {
                log::error!("Unable to sync traits: {err}");
            } else {
                log::info!("Traits added");
            }
        }
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
                                        Ok(Some(payload)) => {
                                            log::info!("Querying minions: {}", payload);
                                            if let Some(msg) = master.lock().await.msg_query(&payload) {
                                                let _ = bcast.send(msg.sendable().unwrap());
                                            }
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

    pub async fn do_heartbeat(master: Arc<Mutex<Self>>) {
        log::trace!("Starting heartbeat");
        let bcast = master.lock().await.broadcast();
        tokio::spawn(async move {
            loop {
                _ = time::sleep(Duration::from_secs(5)).await;
                let mut p = MasterMessage::new(RequestType::Ping, json!(""));
                let mut t = MinionTarget::default();
                t.add_hostname("*");
                p.set_target(t);
                let _ = bcast.send(p.sendable().unwrap());
            }
        });
    }

    pub async fn do_outgoing(master: Arc<Mutex<Self>>, tx: mpsc::Sender<(Vec<u8>, String)>) -> Result<(), SysinspectError> {
        log::trace!("Init outgoing channel");
        let listener = master.lock().await.listener().await?;
        tokio::spawn(async move {
            let bcast = master.lock().await.broadcast();

            loop {
                tokio::select! {
                    // Accept a new connection
                    Ok((socket, _)) = listener.accept() => {
                        let mut bcast_sub = bcast.subscribe();
                        let client_tx = tx.clone();
                        let local_addr = socket.local_addr().unwrap();
                        let (reader, writer) = socket.into_split();
                        let c_master = Arc::clone(&master);
                        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

                        // Task to send messages to the client
                        tokio::spawn(async move {
                            let mut writer = writer;
                            log::info!("Minion {} connected. Ready to send messages.", local_addr.to_string());

                            loop {
                                if let Ok(msg) = bcast_sub.recv().await {
                                    log::trace!("Sending message to minion at {} length of {}", local_addr.to_string(), msg.len());
                                    if writer.write_all(&(msg.len() as u32).to_be_bytes()).await.is_err()
                                        || writer.write_all(&msg).await.is_err()
                                        || writer.flush().await.is_err()
                                    {
                                        if let Err(err) = cancel_tx.send(true) {
                                            log::error!("Sending cancel notification: {err}");
                                        }
                                        break;
                                    }

                                    if c_master.lock().await.to_drop.contains(&local_addr.to_string()) {
                                        c_master.lock().await.to_drop.remove(&local_addr.to_string());
                                        log::info!("Dropping minion: {}", &local_addr.to_string());
                                        if let Err(err) = writer.shutdown().await {
                                            log::error!("Error shutting down outgoing: {err}");
                                        }
                                        if let Err(err) = cancel_tx.send(true) {
                                            log::error!("Sending cancel notification: {err}");
                                        }

                                        return;
                                    }
                                }
                            }
                        });

                        // Task to read messages from the client
                        tokio::spawn(async move {
                            let mut reader = TokioBufReader::new(reader);
                            loop {
                                if *cancel_rx.borrow() {
                                    log::info!("Process terminated");
                                    return;
                                }

                                let mut len_buf = [0u8; 4];
                                if reader.read_exact(&mut len_buf).await.is_err() {
                                    return;
                                }

                                let msg_len = u32::from_be_bytes(len_buf) as usize;
                                let mut msg = vec![0u8; msg_len];
                                if reader.read_exact(&mut msg).await.is_err() {
                                    return;
                                }

                                if client_tx.send((msg, local_addr.to_string())).await.is_err() {
                                    break;
                                }

                            }
                        });
                    }
                }
            }
        });

        Ok(())
    }
}

pub(crate) async fn master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let master = Arc::new(Mutex::new(SysMaster::new(cfg.to_owned())?));
    {
        let mut m = master.lock().await;
        m.init().await?;
    }

    let (client_tx, client_rx) = mpsc::channel::<(Vec<u8>, String)>(100);

    // Start internal fileserver for minions
    fls::start(cfg).await?;

    // Task to read from the FIFO and broadcast messages to clients
    SysMaster::do_fifo(Arc::clone(&master)).await;

    // Handle incoming messages from minions
    SysMaster::do_incoming(Arc::clone(&master), client_rx).await;

    // Accept connections and spawn tasks for each client
    SysMaster::do_outgoing(Arc::clone(&master), client_tx).await?;

    SysMaster::do_heartbeat(Arc::clone(&master)).await;

    // Listen for shutdown signal and cancel tasks
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Received shutdown signal.");
    std::process::exit(0);
}
