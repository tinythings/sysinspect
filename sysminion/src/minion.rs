use crate::{
    callbacks::{ActionResponseCallback, ModelResponseCallback},
    filedata::{MinionFiledata, SensorsFiledata},
    proto::{
        self,
        msg::{CONNECTION_TX, ExitState},
    },
    ptcounter::PTCounter,
    rsa::MinionRSAKeyManager,
};
use chrono::{Duration as ChronoDuration, Utc};
use clap::ArgMatches;
use colored::Colorize;
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libdpq::{DiskPersistentQueue, WorkItem};
use libmodpak::{MODPAK_SYNC_STATE, SysInspectModPakMinion};
use libsensors::sensors::menotify::MeNotifySensor;
use libsensors::service::SensorService;
use libsetup::get_ssh_client_ip;
use libsysinspect::{
    cfg::{
        get_minion_config,
        mmconf::{CFG_MASTER_KEY_PUB, DEFAULT_PORT, MinionConfig, SysInspectConfig},
    },
    context,
    inspector::SysInspectRunner,
    intp::{
        actproc::{modfinder::ModCall, response::ActionResponse},
        inspector::SysInspector,
    },
    mdescr::mspecdef::ModelSpec,
    reactor::{
        evtproc::EventProcessor,
        fmt::{formatter::StringFormatter, kvfmt::KeyValueFormatter},
    },
    rsa::{
        self,
        rotation::{RotationActor, RsaTransportRotator, SignedRotationIntent},
    },
    traits::{self, TraitUpdateRequest, effective_profiles, ensure_master_traits_file, systraits::SystemTraits},
    transport::{
        TransportStore,
        secure_bootstrap::SecureBootstrapSession,
        secure_channel::{SECURE_MAX_FRAME_SIZE, SecureChannel, SecurePeerRole},
    },
    util::{self, dataconv},
};
use libsysproto::{
    MasterMessage, MinionMessage, ProtoConversion,
    errcodes::ProtoErrorCode,
    payload::{ModStatePayload, PayloadType},
    query::{
        MinionQuery, SCHEME_COMMAND,
        commands::{CLUSTER_REBOOT, CLUSTER_REMOVE_MINION, CLUSTER_ROTATE, CLUSTER_SHUTDOWN, CLUSTER_SYNC, CLUSTER_TRAITS_UPDATE},
    },
    rqtypes::{ProtoValue, RequestType},
    secure::{SecureDiagnosticCode, SecureFrame},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml::Value as YamlValue;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
    vec,
};
use tokio::sync::Mutex;
use tokio::{io::AsyncReadExt, time::sleep};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use tokio::{
    net::{TcpStream, tcp::OwnedReadHalf},
    task::JoinHandle,
};
use uuid::Uuid;

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RotationCommandPayload {
    op: String,
    reason: String,
    grace_seconds: u64,
    reconnect: bool,
    reregister: bool,
    intent: SignedRotationIntent,
}
#[derive(Debug)]
pub struct SysMinion {
    cfg: MinionConfig,
    fingerprint: Option<String>,
    kman: MinionRSAKeyManager,

    rstm: Arc<Mutex<OwnedReadHalf>>,
    wstm: Arc<Mutex<OwnedWriteHalf>>,

    filedata: Mutex<MinionFiledata>,

    pub(crate) last_ping: Mutex<Instant>,
    pub(crate) ping_timeout: Duration,

    pt_counter: Mutex<PTCounter>,
    dpq: Arc<DiskPersistentQueue>,
    connected: AtomicBool,
    secure: Mutex<Option<SecureChannel>>,

    minion_id: String,

    pub(crate) sensors_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) sensors_pump: Mutex<Option<JoinHandle<()>>>,
    pub(crate) ping_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) proto_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) stats_task: Mutex<Option<JoinHandle<()>>>,
}

impl SysMinion {
    fn cleanup_empty_sensor_dirs(root: &Path, dir: &Path) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };

        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                Self::cleanup_empty_sensor_dirs(root, &p);
            }
        }

        if dir != root
            && fs::read_dir(dir).map(|mut i| i.next().is_none()).unwrap_or(false)
            && let Err(e) = fs::remove_dir(dir)
        {
            log::debug!("Unable to remove empty sensors dir '{}': {e}", dir.display());
        }
    }

    pub async fn new(cfg: MinionConfig, fingerprint: Option<String>, dpq: Arc<DiskPersistentQueue>) -> Result<Arc<SysMinion>, SysinspectError> {
        log::debug!("Configuration: {cfg:#?}");
        log::debug!("Trying to connect at {}", cfg.master());

        let (rstm, wstm) = TcpStream::connect(cfg.master()).await?.into_split();

        log::debug!("Network bound at {}", cfg.master());
        let instance = SysMinion {
            cfg: cfg.clone(),
            fingerprint,
            kman: MinionRSAKeyManager::new(cfg.root_dir())?,
            rstm: Arc::new(Mutex::new(rstm)),
            wstm: Arc::new(Mutex::new(wstm)),
            filedata: Mutex::new(MinionFiledata::new(cfg.models_dir())?),
            last_ping: Mutex::new(Instant::now()),
            ping_timeout: Duration::from_secs(10),
            pt_counter: Mutex::new(PTCounter::new()),
            dpq,
            connected: AtomicBool::new(false),
            secure: Mutex::new(None),
            minion_id: dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)),
            sensors_task: Mutex::new(None),
            sensors_pump: Mutex::new(None),
            ping_task: Mutex::new(None),
            proto_task: Mutex::new(None),
            stats_task: Mutex::new(None),
        };
        log::debug!("Instance set up with root directory at {}", cfg.root_dir().to_str().unwrap_or_default());
        instance.init()?;

        log::debug!("Initialisation done");

        Ok(Arc::new(instance))
    }

    /// Initialise minion.
    /// This creates all directory structures if none etc.
    fn init(&self) -> Result<(), SysinspectError> {
        log::info!("Initialising minion");
        // Machine id?
        if !self.cfg.machine_id_path().exists() {
            util::write_machine_id(Some(self.cfg.machine_id_path()))?;
        }

        // Place for models
        if !self.cfg.models_dir().exists() {
            log::debug!("Creating directory for the models at {}", self.cfg.models_dir().as_os_str().to_str().unwrap_or_default());
            fs::create_dir_all(self.cfg.models_dir())?;
        }

        // Place for traits.d
        if !self.cfg.traits_dir().exists() {
            log::debug!("Creating directory for the drop-in traits at {}", self.cfg.traits_dir().as_os_str().to_str().unwrap_or_default());
            fs::create_dir_all(self.cfg.traits_dir())?;
        }
        ensure_master_traits_file(&self.cfg)?;

        // Place for trait functions
        if !self.cfg.functions_dir().exists() {
            log::debug!("Creating directory for the custom trait functions at {}", self.cfg.functions_dir().as_os_str().to_str().unwrap_or_default());
            fs::create_dir_all(self.cfg.functions_dir())?;
        }

        // Place for sensors config
        if !self.cfg.sensors_dir().exists() {
            log::debug!("Creating directory for the sensors config at {}", self.cfg.sensors_dir().as_os_str().to_str().unwrap_or_default());
            fs::create_dir_all(self.cfg.sensors_dir())?;
        }

        if !self.cfg.profiles_dir().exists() {
            log::debug!("Creating directory for the synced profiles at {}", self.cfg.profiles_dir().as_os_str().to_str().unwrap_or_default());
            fs::create_dir_all(self.cfg.profiles_dir())?;
        }
        if let Err(err) = self.kman.ensure_transport_state(self.get_minion_id()) {
            log::warn!("Unable to refresh local transport state from RSA identity: {err}");
        }

        let mut out: Vec<String> = vec![];
        for t in traits::get_minion_traits(Some(&self.cfg)).trait_keys() {
            out.push(format!("{}: {}", t.to_owned(), dataconv::to_string(traits::get_minion_traits(None).get(&t)).unwrap_or_default()));
        }
        log::debug!("Minion traits:\n{}", out.join("\n"));
        let profiles = effective_profiles(&self.cfg);
        log::info!(
            "{} {}",
            if profiles.len() == 1 { "Activating profile" } else { "Activating profiles" },
            profiles.iter().map(|name| name.bright_yellow().to_string()).collect::<Vec<String>>().join(", ")
        );

        Ok(())
    }

    /// Stop sensors by aborting their tasks. This is used when the master
    /// disconnects or becomes unresponsive, to stop the sensors and prepare for reconnection.
    pub(crate) async fn stop_sensors(&self) {
        MeNotifySensor::invalidate_all();
        if let Some(h) = self.sensors_pump.lock().await.take() {
            h.abort();
            let _ = h.await;
        }
        if let Some(h) = self.sensors_task.lock().await.take() {
            h.abort();
            let _ = h.await;
        }
    }

    pub(crate) async fn stop_background(&self) {
        if let Some(h) = self.ping_task.lock().await.take() {
            h.abort();
            let _ = h.await;
        }
        if let Some(h) = self.proto_task.lock().await.take() {
            h.abort();
            let _ = h.await;
        }
        if let Some(h) = self.stats_task.lock().await.take() {
            h.abort();
            let _ = h.await;
        }
    }

    /// Display minion info
    pub fn print_info(cfg: &MinionConfig) {
        let mut out: IndexMap<String, String> = IndexMap::new();
        let mut systraits = traits::get_minion_traits_nolog(Some(cfg));

        systraits.put("minion.version".to_string(), json!(env!("CARGO_PKG_VERSION")));
        systraits.put("uri.master".to_string(), json!(cfg.master()));
        systraits.put("uri.fileserver".to_string(), json!(cfg.fileserver()));
        systraits.put("path.models".to_string(), json!(cfg.models_dir()));
        systraits.put("path.functions".to_string(), json!(cfg.functions_dir()));

        for tk in systraits.trait_keys() {
            out.insert(tk.to_owned(), dataconv::to_string(systraits.get(&tk)).unwrap_or_default());
        }

        let mut fmt = KeyValueFormatter::new(json!(out));
        fmt.set_key_title("Trait");
        fmt.set_value_title("Data");

        println!("{}:\n\n{}", "Minion information".bright_green().bold(), fmt.format());
    }

    pub(crate) async fn update_ping(&self) {
        let mut last_ping = self.last_ping.lock().await;
        *last_ping = Instant::now();
    }

    pub fn as_ptr(self: &Arc<Self>) -> Arc<Self> {
        Arc::clone(self)
    }

    /// Get current minion Id
    fn get_minion_id(&self) -> &str {
        &self.minion_id
    }

    /// Talk-back to the master
    pub(crate) async fn request(&self, msg: Vec<u8>) {
        let payload = match self.secure.lock().await.as_mut().map(|secure| secure.seal_bytes(&msg)).transpose() {
            Ok(Some(msg)) => msg,
            Ok(None) => msg,
            Err(err) => {
                log::error!("Failed to encode secure payload to master: {err}");
                let _ = CONNECTION_TX.send(());
                return;
            }
        };

        if let Err(err) = self.write_frame(&payload).await {
            log::warn!("Failed to send message to master: {err}; triggering reconnect");
            let _ = CONNECTION_TX.send(());
        } else {
            log::trace!("To master: {}", String::from_utf8_lossy(&payload));
        }
    }

    /// Write one length-prefixed transport frame to the master connection.
    async fn write_frame(&self, frame: &[u8]) -> Result<(), SysinspectError> {
        let mut stm = self.wstm.lock().await;
        stm.write_all(&(frame.len() as u32).to_be_bytes()).await?;
        stm.write_all(frame).await?;
        stm.flush().await?;
        Ok(())
    }

    /// Read one length-prefixed transport frame from the master connection.
    async fn read_frame(&self) -> Result<Vec<u8>, SysinspectError> {
        let mut stm = self.rstm.lock().await;
        let mut len = [0u8; 4];
        stm.read_exact(&mut len).await?;
        let frame_len = u32::from_be_bytes(len) as usize;
        if frame_len > SECURE_MAX_FRAME_SIZE {
            return Err(SysinspectError::ProtoError(format!("Transport frame exceeds maximum size of {SECURE_MAX_FRAME_SIZE} bytes")));
        }
        let mut frame = vec![0u8; frame_len];
        stm.read_exact(&mut frame).await?;
        Ok(frame)
    }

    /// Mark the specified transport key, or the current managed key, as broken after a failed secure bootstrap.
    fn mark_broken_transport(&self, store: &TransportStore, state: &mut libsysinspect::transport::TransportPeerState, key_id: Option<&str>) {
        let changed = if let Some(key_id) = key_id {
            state.upsert_key(key_id, libsysinspect::transport::TransportKeyStatus::Broken);
            true
        } else {
            state.mark_current_key_broken()
        };
        if changed && let Err(err) = store.save(state) {
            log::warn!("Unable to persist broken transport state: {err}");
        }
    }

    /// Bootstrap a secure session with the master before any normal protocol traffic is allowed.
    pub(crate) async fn bootstrap_secure(&self) -> Result<(), SysinspectError> {
        let store = TransportStore::for_minion(&self.cfg)?;
        let master_pbk = match self.kman.master_public_key()? {
            Some(key) => key,
            None => {
                return Err(SysinspectError::ConfigError(format!(
                    "Trusted master RSA key is missing at {}; secure bootstrap cannot continue",
                    self.cfg.root_dir().join(CFG_MASTER_KEY_PUB).display()
                )));
            }
        };
        let mut state = match store.load()? {
            Some(state) => state,
            None => {
                return Err(SysinspectError::ConfigError(format!(
                    "Managed transport state is missing at {}; secure bootstrap cannot continue",
                    store.state_path().display()
                )));
            }
        };
        let (opening, hello) = match SecureBootstrapSession::open(&state, &self.kman.private_key()?, &master_pbk) {
            Ok(opening) => opening,
            Err(err) => {
                self.mark_broken_transport(&store, &mut state, None);
                return Err(err);
            }
        };
        let opening_key_id = opening.key_id().to_string();
        self.write_frame(
            &serde_json::to_vec(&hello)
                .map_err(|err| SysinspectError::SerializationError(format!("Failed to encode secure bootstrap hello: {err}")))?,
        )
        .await?;
        match serde_json::from_slice::<SecureFrame>(&self.read_frame().await?)
            .map_err(|err| SysinspectError::DeserializationError(format!("Failed to decode secure bootstrap reply: {err}")))?
        {
            SecureFrame::BootstrapAck(ack) => {
                let session = match opening.verify_ack(&state, &ack, &master_pbk) {
                    Ok(session) => session,
                    Err(err) => {
                        self.mark_broken_transport(&store, &mut state, Some(&opening_key_id));
                        return Err(err);
                    }
                };
                state.upsert_key(session.key_id(), libsysinspect::transport::TransportKeyStatus::Active);
                store.save(&state)?;
                *self.secure.lock().await = Some(SecureChannel::new(SecurePeerRole::Minion, &session)?);
                log::info!(
                    "Secure session established with master using key {} and session {}",
                    session.key_id(),
                    session.session_id().unwrap_or_default()
                );
                Ok(())
            }
            SecureFrame::BootstrapDiagnostic(diag) => {
                self.mark_broken_transport(&store, &mut state, Some(&opening_key_id));
                Err(SysinspectError::ProtoError(format!("Master rejected secure bootstrap with {:?}: {}", diag.code, diag.message)))
            }
            _ => {
                self.mark_broken_transport(&store, &mut state, Some(&opening_key_id));
                Err(SysinspectError::ProtoError("Master replied with a non-bootstrap frame during secure bootstrap".to_string()))
            }
        }
    }

    /// A sub-process that checks if a ping is going through. On ping timeout
    /// that would indicate that the Master is either dead or disconnected or not available.
    /// That should kick Minion to start reconnecting.
    pub async fn do_ping_update(self: Arc<Self>, state: Arc<ExitState>) -> Result<(), SysinspectError> {
        let reconnect_sender = CONNECTION_TX.clone();
        let h = tokio::spawn({
            let this = self.clone();
            async move {
                loop {
                    sleep(Duration::from_secs(1)).await;

                    // Do not reconnect mid-sync.
                    if MODPAK_SYNC_STATE.is_syncing().await {
                        this.update_ping().await; // keep watchdog calm
                        continue;
                    }

                    if this.last_ping.lock().await.elapsed() > this.ping_timeout {
                        log::warn!("Master seems unresponsive, triggering reconnect.");
                        let _ = reconnect_sender.send(());
                        state.exit.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        *self.ping_task.lock().await = Some(h);
        Ok(())
    }

    /// Set the minion connected flag to true or false, which indicates whether the minion is currently connected to the master or not.
    /// This is used to bounce self from already connected state. If the connected flag is set to false, then the Minion quits.
    fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    /// Check if the minion connected flag is currently set to true, which indicates that the minion is currently connected to the master.
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Start sensors based on the configuration, and pump their events to the reactor.
    /// This is a separate async task that runs in parallel with the main loop, and is responsible
    /// for keeping the sensors running and their events flowing to the reactor.
    pub async fn do_sensors(self: Arc<Self>, sensors: SensorsFiledata) -> Result<(), SysinspectError> {
        // prune stale local files
        let sroot = self.cfg.sensors_dir();
        for rel in sensors.stale_paths() {
            let pth = sroot.join(rel);
            if pth.is_file() {
                match fs::remove_file(&pth) {
                    Ok(_) => log::info!("Removed stale sensor file {}", format!("\"{}\"", pth.display()).yellow()),
                    Err(e) => log::error!("Failed to remove stale sensor file '{}': {e}", pth.display()),
                }
            }
        }
        Self::cleanup_empty_sensor_dirs(&sroot, &sroot);

        // download
        for f in sensors.files().keys() {
            log::info!("Sensor file '{}' with checksum {} needs to be downloaded or updated.", f, sensors.files().get(f).unwrap_or(&"".to_string()));

            match self.as_ptr().download_file(f).await {
                Ok(content) => {
                    let Some(rel) = sensors.local_rel_path(f) else {
                        log::warn!("Skipping unsafe sensor path from master payload while writing file: '{}'", f);
                        continue;
                    };
                    let pth = self.cfg.sensors_dir().join(rel);

                    if let Some(parent) = pth.parent() {
                        if !parent.exists()
                            && let Err(e) = fs::create_dir_all(parent)
                        {
                            log::error!("Failed to create directories for '{}': {e}", pth.display());
                            continue;
                        }
                        if let Err(e) = fs::write(&pth, content) {
                            log::error!("Failed to write sensor file '{}': {e}", pth.display());
                        }
                    }
                }
                Err(e) => log::error!("Failed to download sensor file '{}': {e}", f),
            }
        }

        self.stop_sensors().await;
        log::info!("Restarting sensors service");

        // Load spec before spawning anything
        let spec = match libsensors::load(self.cfg.sensors_dir().as_path()) {
            Ok(spec) => spec,
            Err(e) => {
                log::error!("Failed to load sensors spec: {e}");
                return Ok(());
            }
        };

        libsysinspect::reactor::handlers::registry::init_handlers();

        let mut events = EventProcessor::new();
        if let Some(cfg) = spec.events_config() {
            events = events.set_config(Arc::new(cfg.clone()), None);
        }

        let events = Arc::new(Mutex::new(events));

        // Spawn pump and store handle immediately
        let events_pump = events.clone();
        let pump_handle = tokio::spawn(async move {
            loop {
                {
                    let mut ep = events_pump.lock().await;
                    ep.process(true).await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        });
        *self.sensors_pump.lock().await = Some(pump_handle);

        // Spawn service task and store it
        let sensors_task = {
            let mut service = SensorService::new(spec);
            service.set_event_processor(events.clone());
            service.spawn()
        };
        *self.sensors_task.lock().await = Some(sensors_task);

        Ok(())
    }

    pub async fn do_stats_update(self: Arc<Self>) -> Result<(), SysinspectError> {
        let this = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;
                let mut ptc = this.pt_counter.lock().await;
                ptc.update_stats();
            }
        });

        *self.stats_task.lock().await = Some(handle);
        Ok(())
    }

    pub async fn do_proto(self: Arc<Self>) -> Result<(), SysinspectError> {
        let this = self.clone();

        let handle = tokio::spawn(async move {
            loop {
                let msg = match this.read_frame().await {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::warn!("Proto read failed: {err}; triggering reconnect");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }
                };

                let msg = match this.secure.lock().await.as_mut().map(|secure| secure.open_bytes(&msg)).transpose() {
                    Ok(Some(msg)) => msg,
                    Ok(None) => msg,
                    Err(err) => {
                        log::error!("Failed to decode secure frame from master: {err}");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }
                };

                let msg = match proto::msg::payload_to_msg(msg.clone()) {
                    Ok(msg) => msg,
                    Err(err) => {
                        if let Ok(diag) = proto::msg::payload_to_diag(&msg) {
                            log::warn!("Master rejected secure bootstrap: {}", diag.message);
                            if matches!(diag.code, SecureDiagnosticCode::UnsupportedVersion) {
                                let _ = CONNECTION_TX.send(());
                                break;
                            }
                            continue;
                        }
                        log::error!("Error getting network payload as message: {err}");
                        continue;
                    }
                };

                log::trace!("Received Master message: {msg:#?}");
                // Any successfully decoded inbound frame proves the master link is alive.
                // Keep watchdog independent from outbound write pressure.
                this.update_ping().await;

                match msg.req_type() {
                    RequestType::Add => log::debug!("Master accepts registration"),

                    RequestType::Reconnect => {
                        log::warn!("Master requires reconnection: {}", msg.payload());
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::Remove => {
                        log::debug!("Master asks to unregister");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::Command => {
                        log::debug!("Master sends a command");
                        match msg.get_retcode() {
                            ProtoErrorCode::Success => {
                                let scheme = msg.target().scheme().to_string();

                                if scheme.starts_with(SCHEME_COMMAND) {
                                    this.as_ptr().call_internal_command(&scheme, msg.target().context()).await;
                                    continue;
                                }

                                if let Err(err) = this.as_ptr().dpq.add(WorkItem::MasterCommand(msg.to_owned())) {
                                    log::error!("Failed to enqueue master command: {err}");
                                } else {
                                    log::info!("Scheduled master command: {}", msg.target().scheme());
                                }
                            }

                            ProtoErrorCode::AlreadyConnected => {
                                if !this.is_connected() {
                                    log::error!("Another minion from this machine is already connected");
                                    let _ = CONNECTION_TX.send(());
                                    break;
                                }
                            }

                            ret => log::debug!("Return code {ret:?} not yet implemented"),
                        }
                    }

                    RequestType::Traits => {
                        if this.get_minion_id() != msg.target().id() {
                            log::debug!("Traits request for {}, not me; dropping", msg.target().id());
                            continue;
                        }

                        match this.as_ptr().send_traits().await {
                            Ok(_) => {
                                log::info!("Connected");
                                this.set_connected(true);
                            }
                            Err(err) => log::error!("Unable to send traits: {err}"),
                        }
                    }

                    RequestType::AgentUnknown => {
                        let pbk_pem = dataconv::as_str(Some(msg.payload()).cloned());
                        let (_, pbk) = match rsa::keys::from_pem(None, Some(&pbk_pem)) {
                            Ok(val) => val,
                            Err(err) => {
                                log::error!("Failed to parse PEM: {err}");
                                let _ = CONNECTION_TX.send(());
                                break;
                            }
                        };

                        let pbk = match pbk {
                            Some(key) => key,
                            None => {
                                log::error!("No public key found in PEM");
                                let _ = CONNECTION_TX.send(());
                                break;
                            }
                        };

                        let fpt = match rsa::keys::get_fingerprint(&pbk) {
                            Ok(fp) => fp,
                            Err(err) => {
                                log::error!("Failed to get fingerprint: {err}");
                                let _ = CONNECTION_TX.send(());
                                break;
                            }
                        };

                        log::error!("Minion is not registered");
                        log::info!("Master fingerprint: {fpt}");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::Ping => {
                        let p = msg.payload();
                        match serde_json::from_value::<ProtoValue>(p.clone()) {
                            Ok(ProtoValue::PingTypeGeneral) => {
                                let (loadavg, is_done, doneids, io_bps, cpu_usage) = {
                                    let mut ptc = this.pt_counter.lock().await;
                                    let (l, d, i, io, cpu) =
                                        (ptc.get_loadaverage(), ptc.is_done(), ptc.get_done(), ptc.get_io_bps(), ptc.get_cpu_usage());
                                    if d {
                                        ptc.flush_done();
                                    }
                                    (l, d, i, io, cpu)
                                };

                                let pl = json!({
                                    "ld": loadavg,
                                    "cd": if is_done { doneids } else { vec![] },
                                    "dbps": io_bps,
                                    "cpu": cpu_usage,
                                });

                                this.request(proto::msg::get_pong(ProtoValue::PingTypeGeneral, Some(pl))).await;
                            }

                            Ok(ProtoValue::PingTypeDiscovery) => {
                                log::debug!("Received discovery ping from master");
                                this.request(proto::msg::get_pong(ProtoValue::PingTypeDiscovery, None)).await;
                            }

                            Err(err) => log::warn!("Invalid ping payload `{}`: {}", p, err),
                        }
                    }

                    RequestType::ByeAck => {
                        log::info!("Master confirmed shutdown");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::SensorsSyncResponse => {
                        log::info!("Received sensors sync response from master");
                        let sensors = SensorsFiledata::from_payload(msg.payload().clone(), this.cfg.sensors_dir()).unwrap_or_else(|err| {
                            log::error!("Failed to parse sensors payload: {err}");
                            SensorsFiledata::default()
                        });

                        if let Err(err) = this.as_ptr().do_sensors(sensors).await {
                            log::error!("Failed to start sensors: {err}");
                        }
                    }

                    _ => log::error!("Unknown request type"),
                }
            }
        });

        *self.proto_task.lock().await = Some(handle);
        Ok(())
    }

    pub async fn send_traits(self: Arc<Self>) -> Result<(), SysinspectError> {
        let fresh_traits = SystemTraits::new(self.cfg.clone(), false);
        let mut r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Traits, fresh_traits.to_json_value()?);
        r.set_sid(MINION_SID.to_string());
        self.request(r.sendable().map_err(|e| {
            log::error!("Error preparing traits message: {e}");
            e
        })?)
        .await;
        Ok(())
    }

    /// Send ehlo
    pub async fn send_ehlo(self: Arc<Self>) -> Result<(), SysinspectError> {
        let fresh_traits = SystemTraits::new(self.cfg.clone(), false);
        let mut r = MinionMessage::new(dataconv::as_str(fresh_traits.get(traits::SYS_ID)), RequestType::Ehlo, fresh_traits.to_json_value()?);
        r.set_sid(MINION_SID.to_string());

        log::info!("Ehlo on {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send registration request
    pub async fn send_registration(self: Arc<Self>, pbk_pem: String) -> Result<(), SysinspectError> {
        let r = MinionMessage::new(dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)), RequestType::Add, json!(pbk_pem));

        log::info!("Registration request to {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send callback to the master on the results
    pub async fn send_callback(self: Arc<Self>, ar: ActionResponse) -> Result<(), SysinspectError> {
        log::debug!("Sending sync callback on {}", ar.aid());
        log::debug!("Callback: {ar:#?}");
        self.request(MinionMessage::new(self.get_minion_id().to_string(), RequestType::Event, json!(ar)).sendable()?).await;
        Ok(())
    }

    /// Send finalisation marker callback to the master on the results
    pub async fn send_fin_callback(self: Arc<Self>, ar: ActionResponse) -> Result<(), SysinspectError> {
        log::debug!("Sending fin sync callback on {}", ar.aid());
        self.request(MinionMessage::new(self.get_minion_id().to_string(), RequestType::ModelEvent, json!(ar)).sendable()?).await;
        Ok(())
    }

    pub(crate) async fn send_sensors_sync(self: Arc<Self>) -> Result<(), SysinspectError> {
        log::info!("Sending sensors sync callback for cycle");
        let mut r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::SensorsSyncRequest, json!({}));
        r.set_sid(MINION_SID.to_string());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send bye message
    pub async fn send_bye(self: Arc<Self>) {
        let r = MinionMessage::new(
            dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)),
            RequestType::Bye,
            json!(MINION_SID.to_string()),
        );

        log::info!("Goodbye to {}", self.cfg.master());
        match r.sendable() {
            Ok(msg) => self.request(msg).await,
            Err(e) => log::error!("Failed to send bye message: {e}"),
        }
    }

    async fn apply_rotation_command(self: Arc<Self>, context: &str) -> Result<(), SysinspectError> {
        let payload: RotationCommandPayload =
            serde_json::from_str(context).map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse rotate payload: {err}")))?;
        if payload.op != "rotate" {
            return Err(SysinspectError::InvalidQuery(format!("Unsupported rotate operation {}", payload.op)));
        }

        let master_pbk = self.kman.master_public_key()?.ok_or_else(|| {
            SysinspectError::ConfigError(format!(
                "Trusted master RSA key is missing at {}; rotate cannot proceed",
                self.cfg.root_dir().join(CFG_MASTER_KEY_PUB).display()
            ))
        })?;
        let master_fp = rsa::keys::get_fingerprint(&master_pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?;
        let minion_fp = self.kman.get_pubkey_fingerprint()?;
        let mut rotator = RsaTransportRotator::new(
            RotationActor::Minion,
            TransportStore::for_minion(&self.cfg)?,
            self.get_minion_id(),
            &master_fp,
            &minion_fp,
            libsysproto::secure::SECURE_PROTOCOL_VERSION,
        )?;

        let overlap = ChronoDuration::seconds(payload.grace_seconds as i64);
        let _ = rotator.retire_elapsed_keys(Utc::now(), overlap)?;
        let ticket = rotator.execute_signed_intent_with_overlap(&payload.intent, &master_pbk, overlap)?;
        let _ = rotator.retire_elapsed_keys(Utc::now(), overlap)?;

        if payload.reregister {
            let _ = self.kman.ensure_transport_state(self.get_minion_id())?;
        }

        log::info!(
            "Applied transport rotation {} for {} at {}",
            ticket.result().active_key_id().bright_yellow(),
            self.get_minion_id().bright_green(),
            ticket.result().rotated_at().to_rfc3339().bright_blue()
        );

        if payload.reconnect {
            self.send_bye().await;
        }

        Ok(())
    }

    /// Download a file from master
    async fn download_file(self: Arc<Self>, fname: &str) -> Result<Vec<u8>, SysinspectError> {
        async fn fetch_file(url: &str, filename: &str) -> Result<String, SysinspectError> {
            let url = format!("http://{}/{}", url.trim_end_matches('/'), filename.to_string().trim_start_matches('/'));
            let rsp = match reqwest::get(url.to_owned()).await {
                Ok(rsp) => rsp,
                Err(err) => {
                    return Err(SysinspectError::MinionGeneralError(format!("Unable to get file at {url}: {err}")));
                }
            };

            Ok(match rsp.status() {
                reqwest::StatusCode::OK => match rsp.text().await {
                    Ok(data) => data,
                    Err(err) => {
                        return Err(SysinspectError::MinionGeneralError(format!("Unable to get text from the file: {err}")));
                    }
                },
                reqwest::StatusCode::NOT_FOUND => return Err(SysinspectError::MinionGeneralError("File not found".to_string())),
                _ => return Err(SysinspectError::MinionGeneralError("Unknown status".to_string())),
            })
        }
        let addr = self.cfg.fileserver();
        let fname = fname.to_string();
        let h = tokio::spawn(async move {
            match fetch_file(&addr, &fname).await {
                Ok(data) => {
                    log::debug!("Filename: {fname} contains {} bytes", data.len());
                    Some(data.into_bytes())
                }
                Err(err) => {
                    log::error!("Error while downloading file {fname}: {err}");
                    None
                }
            }
        });

        if let Ok(Some(data)) = h.await {
            return Ok(data);
        }

        Err(SysinspectError::MinionGeneralError("File was not downloaded".to_string()))
    }

    /// Launch sysinspect
    async fn launch_sysinspect(self: Arc<Self>, cycle_id: &str, scheme: &str, msp: &ModStatePayload, context: &str) {
        // Get the query first
        let mqr = match MinionQuery::new(scheme) {
            Ok(mqr) => mqr,
            Err(err) => {
                log::error!("Query error: {err}");
                return;
            }
        };

        // Auto-sync all data files
        let mut dirty = false;
        for (uri_file, fcs) in msp.files() {
            let dst = self
                .as_ptr()
                .cfg
                .models_dir()
                .join(uri_file.trim_start_matches(&format!("/{}", msp.models_root())).strip_prefix("/").unwrap_or_default());

            if self.as_ptr().filedata.lock().await.check_sha256(uri_file.to_owned(), fcs.to_owned(), true) {
                continue;
            }
            log::info!("Auto-updating {uri_file}");

            match self.as_ptr().download_file(uri_file).await {
                Ok(data) => {
                    let dst_dir = match dst.parent() {
                        Some(p) => p.to_path_buf(),
                        None => {
                            log::error!("Unable to determine parent directory for file: {}", dst.display());
                            return;
                        }
                    };

                    if !dst_dir.exists() {
                        log::debug!("Creating directory: {}", dst_dir.display());
                        if let Err(err) = fs::create_dir_all(&dst_dir) {
                            log::error!("Unable to create directories for model download: {err}");
                            return;
                        }
                    }

                    log::debug!("Saving URI {uri_file} as {}", dst_dir.display());
                    if let Err(err) = fs::write(&dst, data) {
                        log::error!("Unable to save downloaded file to {}: {err}", dst.display());
                        return;
                    }
                    dirty = true;
                }
                Err(_) => todo!(),
            }
        }
        if dirty {
            self.as_ptr().filedata.lock().await.init();
        }

        // Run the model
        {
            log::debug!("Launching model for sysinspect for: {scheme}");
            let mqr_guard = mqr.lock().await;

            let mut sr = SysInspectRunner::new(&self.cfg);

            sr.set_model_path(self.as_ptr().cfg.models_dir().join(mqr_guard.target()).to_str().unwrap_or_default());
            sr.set_state(mqr_guard.state());
            sr.set_entities(mqr_guard.entities());
            sr.set_checkbook_labels(mqr_guard.checkbook_labels());
            sr.set_traits(traits::get_minion_traits(None));
            sr.set_context(context::get_context(context));

            sr.add_action_callback(Box::new(ActionResponseCallback::new(self.as_ptr(), cycle_id)));
            sr.add_model_callback(Box::new(ModelResponseCallback::new(self.as_ptr(), cycle_id)));

            match tokio::task::spawn_blocking(move || futures::executor::block_on(sr.start())).await {
                Ok(()) => {
                    log::debug!("Task {} finished", cycle_id);
                }
                Err(e) => {
                    log::error!("Blocking task crashed: {e}");
                }
            };
            self.as_ptr().pt_counter.lock().await.dec(cycle_id);
        }
    }

    /// Calls internal command
    async fn call_internal_command(self: Arc<Self>, cmd: &str, context: &str) {
        let cmd = cmd.strip_prefix(SCHEME_COMMAND).unwrap_or_default();
        match cmd {
            CLUSTER_SHUTDOWN => {
                log::info!("Requesting minion shutdown from a master");
                self.as_ptr().send_bye().await;
                std::process::exit(0);
            }
            CLUSTER_REBOOT => {
                log::warn!("Command \"reboot\" is not implemented yet");
            }
            CLUSTER_ROTATE => match self.clone().apply_rotation_command(context).await {
                Ok(_) => {
                    if !context.is_empty() {
                        let _ = CONNECTION_TX.send(());
                    }
                }
                Err(err) => log::error!("Failed to apply rotate command: {err}"),
            },
            CLUSTER_REMOVE_MINION => {
                log::info!("{} from the master", "Unregistering".bright_red().bold());
                self.as_ptr().send_bye().await;
            }
            CLUSTER_SYNC => {
                log::info!("Syncing the minion with the master");
                if let Err(e) = ensure_master_traits_file(&self.cfg) {
                    log::error!("Failed to ensure master-managed traits file: {e}");
                }
                if let Err(e) = SysInspectModPakMinion::new(self.cfg.clone()).sync().await {
                    log::error!("Failed to sync minion with master: {e}");
                }
                if let Err(e) = self.as_ptr().send_traits().await {
                    log::error!("Failed to sync traits with master: {e}");
                }
                let _ = self.as_ptr().send_sensors_sync().await;
            }
            CLUSTER_TRAITS_UPDATE => match TraitUpdateRequest::from_context(context) {
                Ok(update) => match update.apply(&self.cfg) {
                    Ok(_) => {
                        let summary = if update.op() == "reset" {
                            "all master-managed traits".bright_yellow().to_string()
                        } else if update.op() == "unset" {
                            update.traits().keys().map(|key| key.yellow().to_string()).collect::<Vec<String>>().join(", ")
                        } else {
                            update
                                .traits()
                                .iter()
                                .map(|(key, value)| {
                                    format!("{}: {}", key.yellow(), dataconv::to_string(Some(value.clone())).unwrap_or_default().bright_yellow())
                                })
                                .collect::<Vec<String>>()
                                .join(", ")
                        };
                        let label = match update.op() {
                            "set" => "Set traits",
                            "unset" => "Unset traits",
                            "reset" => "Reset traits",
                            _ => "Updated traits",
                        };
                        log::info!("{}: {}", label, summary);
                        if let Err(err) = self.as_ptr().send_traits().await {
                            log::error!("Failed to sync traits with master: {err}");
                        }
                    }
                    Err(err) => log::error!("Failed to apply traits update: {err}"),
                },
                Err(err) => log::error!("Failed to parse traits update payload: {err}"),
            },
            _ => {
                log::warn!("Unknown command: {cmd}");
            }
        }
    }

    async fn dispatch(self: Arc<Self>, cmd: MasterMessage) {
        log::debug!("Dispatching message: {cmd:#?}");

        if cmd.cycle().is_empty() {
            log::error!("Cycle ID is empty!");
            return;
        }

        let tgt = cmd.target();

        // Is command minion-specific?
        if !tgt.id().is_empty() && tgt.id().ne(&self.get_minion_id()) {
            log::debug!("Command was dropped as it was specifically addressed for another minion");
            return;
        } else if tgt.id().is_empty() {
            let traits = traits::get_minion_traits(None);

            // Is matching this host?
            let mut skip = true;
            let hostname = dataconv::as_str(traits.get("system.hostname.fqdn")); // Fully qualified domain name or short, if your network is crap
            if !hostname.is_empty() {
                for hq in tgt.hostnames() {
                    if let Ok(hq) = glob::Pattern::new(&hq)
                        && hq.matches(&hostname)
                    {
                        skip = false;
                        break;
                    }
                }
                if skip {
                    log::debug!("Command was dropped as it is specifically targeting different hosts");
                    return;
                }
            }

            // Can match the host, but might not match by traits.
            // For example, web* can match "webschool.com" or "webshop.com",
            // but traits for those hosts might be different.
            let tq = tgt.traits_query();
            if !tq.is_empty() {
                match traits::parse_traits_query(tq) {
                    Ok(q) => {
                        match traits::to_typed_query(q) {
                            Ok(tpq) => {
                                if !traits::matches_traits(tpq, traits::get_minion_traits(None)) {
                                    log::debug!("Command was dropped as it does not match the traits");
                                    return;
                                }
                            }
                            Err(e) => log::error!("{e}"),
                        };
                    }
                    Err(e) => log::error!("{e}"),
                };
            }
        } // else: this minion is directly targeted by its Id.

        log::debug!("Through. {:?}", cmd.payload());
        self.as_ptr().pt_counter.lock().await.inc(cmd.cycle());

        match PayloadType::try_from(cmd.payload().clone()) {
            Ok(PayloadType::ModelOrStatement(pld)) => {
                if cmd.target().scheme().starts_with(SCHEME_COMMAND) {
                    self.as_ptr().call_internal_command(cmd.target().scheme(), cmd.target().context()).await;
                } else {
                    self.as_ptr().launch_sysinspect(cmd.cycle(), cmd.target().scheme(), &pld, cmd.target().context()).await;
                    log::debug!("Command dispatched");
                    log::debug!("Command payload: {pld:#?}");
                }
            }
            Ok(PayloadType::Undef(pld)) => {
                log::error!("Rejected due to the undefined payload in the command: {pld:#?}");
            }
            Err(err) => {
                log::error!("Error dispatching command: {err}");
            }
        }
    }

    #[cfg(test)]
    pub fn set_ping_timeout(&mut self, d: Duration) {
        self.ping_timeout = d;
    }

    /// Install a secure steady-state channel for tests that exercise encrypted transport writes.
    #[cfg(test)]
    pub async fn set_secure_channel(&self, channel: SecureChannel) {
        *self.secure.lock().await = Some(channel);
    }
}

/// Constructs and starts an actual minion
pub(crate) async fn _minion_instance(cfg: MinionConfig, fingerprint: Option<String>, dpq: Arc<DiskPersistentQueue>) -> Result<(), SysinspectError> {
    let state = Arc::new(ExitState::new());
    // Subscribe BEFORE any await (TcpStream::connect happens in SysMinion::new)
    // and keep this receiver for the entire instance lifetime.
    let mut reconnect_rx = CONNECTION_TX.subscribe();

    let modpak = SysInspectModPakMinion::new(cfg.clone());
    let minion = SysMinion::new(cfg.clone(), fingerprint, dpq).await?;
    let m = minion.as_ptr();

    let runner = m.as_ptr().dpq.clone().start_ack({
        let m = m.clone();
        move |_job_id, item| {
            let m = m.clone();
            async move {
                match item {
                    WorkItem::MasterCommand(cmd) | WorkItem::EventCommand(cmd) => {
                        while MODPAK_SYNC_STATE.is_syncing().await {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        m.clone().dispatch(cmd).await;
                        Ok(())
                    }
                }
            }
        }
    });

    // Messages
    if minion.fingerprint.is_some() {
        minion.as_ptr().do_proto().await?;
        minion.as_ptr().send_registration(minion.kman.get_pubkey_pem()).await?;
    } else {
        if let Err(err) = minion.bootstrap_secure().await {
            log::error!("Unable to bootstrap secure transport: {err}");
            return Err(err);
        }
        minion.as_ptr().do_proto().await?;
        minion.as_ptr().send_ehlo().await?;
        if cfg.autosync_startup() {
            tokio::select! {
                sync_res = modpak.sync() => {
                    sync_res?;
                }
                sig = reconnect_rx.recv() => {
                    match sig {
                        Ok(_) => state.exit.store(true, Ordering::Relaxed),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("Missed {n} reconnect notification(s) during startup sync; exiting minion instance.");
                            state.exit.store(true, Ordering::Relaxed);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            log::warn!("Reconnect channel closed during startup sync; exiting minion instance.");
                            state.exit.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
        } else {
            log::warn!("Module auto-sync {} is disabled. Call cluster sync to force modules sync.", "on startup".bright_yellow());
        }
    }

    if state.exit.load(Ordering::Relaxed) {
        runner.abort();
        let _ = runner.await;
        return Ok(());
    }

    // Send sensors sync request
    minion.as_ptr().send_sensors_sync().await?;

    minion.as_ptr().do_ping_update(state.clone()).await?;
    minion.as_ptr().do_stats_update().await?;

    // Keeps client running
    while !state.exit.load(std::sync::atomic::Ordering::Relaxed) {
        tokio::select! {
            sig = reconnect_rx.recv() => {
                match sig {
                    Ok(_) => state.exit.store(true, Ordering::Relaxed),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Missed {n} reconnect notification(s) in main loop; exiting minion instance.");
                        state.exit.store(true, Ordering::Relaxed);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Reconnect channel closed in main loop; exiting minion instance.");
                        state.exit.store(true, Ordering::Relaxed);
                    }
                }
            }
            _ = sleep(tokio::time::Duration::from_millis(200)) => {}
        }
    }

    minion.as_ptr().stop_sensors().await;
    minion.as_ptr().stop_background().await;

    runner.abort();
    let _ = runner.await;

    Ok(())
}

pub async fn minion(cfg: MinionConfig, fp: Option<String>) {
    let mut reconnect_rx = CONNECTION_TX.subscribe();
    let mut ra = 0;
    let dpq = match DiskPersistentQueue::open(cfg.root_dir().join("pending-tasks")) {
        Ok(dpq) => Arc::new(dpq),
        Err(e) => {
            log::error!("Failed to open disk persistent queue: {e}");
            log::error!(
                "Is there another minion running? If not, delete the {} directory and try again.",
                cfg.root_dir().join("pending-tasks").to_str().unwrap_or_default().bright_yellow()
            );
            std::process::exit(1);
        }
    };
    SysInspectRunner::set_dpq(dpq.clone());
    loop {
        log::info!("Starting minion instance...");

        let c_cfg = cfg.clone();
        let c_fp = fp.clone();
        let c_dpq = dpq.clone();
        let mut mhdl = tokio::spawn(async move { _minion_instance(c_cfg, c_fp, c_dpq).await });

        tokio::select! {
            res = &mut mhdl => {
                match res {
                    Ok(Ok(_)) => log::info!("Minion instance ended gracefully, reconnecting..."),
                    Ok(Err(e)) => log::error!("Minion encountered an error: {e:?}"),
                    Err(e) => log::error!("Minion task panicked or was cancelled: {e:?}"),
                }
            }
            sig = reconnect_rx.recv() => {
                match sig {
                    Ok(_) => {
                        log::warn!("Reconnect signal received; aborting current minion instance.");
                        let _ = mhdl.await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Missed {n} reconnect notification(s) in supervisor loop; aborting current minion instance.");
                        let _ = mhdl.await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Reconnect channel closed in supervisor loop; waiting for minion instance task.");
                        let _ = mhdl.await;
                    }
                }
            }
        }

        if !cfg.reconnect() {
            log::warn!("Reconnect is disabled, exiting...");
            std::process::exit(1);
        } else {
            ra += 1;
            log::info!("Reconnect attempt: {ra}");
            if cfg.reconnect_freq() > 0 && ra > cfg.reconnect_freq() {
                log::warn!("Too many reconnect attempts, exiting...");
                std::process::exit(1);
            }
        }

        let interval = cfg.reconnect_interval();
        log::info!("Reconnecting in {interval} seconds...");
        sleep(Duration::from_secs(interval)).await;
    }
}

/// Setup minion
///
/// This is a quick setup for the minion. It creates all directories and files
/// required for the minion to run. It does not start the minion, but only prepares
/// it for the first run.
pub(crate) fn setup(args: &ArgMatches) -> Result<(), SysinspectError> {
    let alt_dir = args.get_one::<String>("directory").unwrap_or(&"".to_string()).to_string();
    let mut dir = PathBuf::from(&alt_dir);
    if !alt_dir.is_empty() {
        if !dir.exists() {
            log::info!("Creating directory {}", dir.to_str().unwrap_or_default());
            fs::create_dir_all(&dir)?;
        } else if !dir.is_dir() {
            return Err(SysinspectError::ConfigError(format!("{} is not a directory", dir.to_str().unwrap_or_default())));
        }

        dir = fs::canonicalize(dir)?;
    }

    if args.get_flag("with-default-config") {
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(args.get_one::<String>("master-addr").unwrap_or(&get_ssh_client_ip().unwrap_or_default()));
        cfg.set_master_port(DEFAULT_PORT);

        if !alt_dir.is_empty() {
            cfg.set_root_dir(dir.to_str().unwrap_or_default());
        }

        let cp = PathBuf::from("sysinspect.conf");
        if !cp.exists() {
            log::info!("Creating default config file at {}", cp.to_str().unwrap_or_default());
            fs::write(cp, SysInspectConfig::default().set_minion_config(cfg).to_yaml())?;
        } else {
            return Err(SysinspectError::ConfigError("Config file already exists. Delete it, perhaps?..".to_string()));
        }
    }

    libsetup::mnsetup::MinionSetup::new().set_config(get_minion_config(None)?).set_alt_dir(dir.to_str().unwrap_or_default().to_string()).setup()
}

/// Launch a module
pub(crate) fn launch_module(cfg: MinionConfig, args: &ArgMatches) -> Result<(), SysinspectError> {
    let name = args.get_one::<String>("name").ok_or(SysinspectError::ConfigError("Module name is required".to_string()))?;
    let mut modcaller = ModCall::default().set_module_ns(name, cfg.sharelib_dir());
    let _ = SysInspector::new(ModelSpec::default(), Some(cfg.sharelib_dir()), IndexMap::new()); // That will fail (and it is OK), but it will set sharelib for the module caller

    for (k, v) in args
        .get_many::<(String, String)>("args")
        .unwrap_or_default()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<(String, String)>>()
    {
        modcaller.add_kwargs(k.to_string(), YamlValue::String(v));
    }

    for o in args.get_many::<Vec<String>>("opts").unwrap_or_default().flatten().cloned().collect::<Vec<String>>() {
        modcaller.add_opt(o);
    }

    let out = modcaller.run()?.unwrap_or_default().response.data().unwrap_or_default();
    if !out.is_null() {
        println!("\n{}", KeyValueFormatter::new(out).format());
        return Ok(());
    } else {
        log::debug!("No data returned from the module {name}");
    }

    Ok(())
}
