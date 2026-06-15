use crate::{
    callbacks::{ActionResponseCallback, ModelResponseCallback},
    filedata::{MinionFiledata, SensorsFiledata},
    inbound_cmd::{InboundCommandClaim, InboundCommandLedger, InboundCommandState},
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
use libsensors::sensors::SensorCtx;
use libsensors::sensors::menotify::MeNotifySensor;
use libsensors::service::SensorService;
use libsetup::get_ssh_client_ip;
use libsetup::mnsetup::ensure_minion_tree;
use libsysinspect::{
    cfg::{
        get_minion_config,
        mmconf::{CFG_MASTER_KEY_PUB, CFG_PENDING_TASKS_ROOT, DEFAULT_PORT, MinionConfig, MinionOfflineMode, SysInspectConfig},
    },
    console::{
        ConsoleMinionLogRequest, ConsoleMinionLogSnapshot, ConsoleMinionProcessSignalRequest, ConsoleMinionTopRequest,
        ConsoleMinionUpgradeSelfRequest, MinionCommandReply,
    },
    context,
    inspector::SysInspectRunner,
    intp::{
        actproc::{
            modfinder::ModCall,
            response::{ActionModResponse, ActionResponse, ConstraintResponse},
        },
        inspector::SysInspector,
    },
    journal::Journal,
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
    util::{self, dataconv, top::collect_top_snapshot},
};
use libsysproto::{
    MasterMessage, MinionMessage, ProtoConversion,
    errcodes::ProtoErrorCode,
    payload::{ModStatePayload, PayloadType, RegistrationReply},
    query::{
        MinionQuery, SCHEME_COMMAND,
        commands::{
            CLUSTER_MINION_LOGS, CLUSTER_MINION_PROCESS_SIGNAL, CLUSTER_MINION_RECONNECT, CLUSTER_MINION_SHUTDOWN, CLUSTER_MINION_TOP,
            CLUSTER_MINION_UPGRADE_SELF, CLUSTER_REBOOT, CLUSTER_RECONNECT, CLUSTER_REMOVE_MINION, CLUSTER_ROTATE, CLUSTER_SHUTDOWN, CLUSTER_SYNC,
            CLUSTER_TRAITS_UPDATE,
        },
    },
    replay::{ReplayIdentity, replay_identity_from_minion_bytes},
    rqtypes::{OutboundMessageClass, ProtoValue, RequestType},
    secure::{SecureDiagnosticCode, SecureFrame},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml::Value as YamlValue;
use std::{
    collections::VecDeque,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    sync::RwLock,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
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

const RUNNER_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
const LOG_RING_CAPACITY: usize = 2000;

/// In-memory unified ring buffer for log capture.
/// Both stdout and stderr streams are merged in insertion order
/// with an atomic monotonic sequence number.
/// Used as fallback when log files don't exist on disk.
pub struct LogRingBuffers {
    entries: RwLock<VecDeque<(u64, String)>>,
    next_seq: AtomicU64,
    capacity: usize,
}

impl LogRingBuffers {
    pub fn new(capacity: usize) -> Self {
        Self { entries: RwLock::new(VecDeque::with_capacity(capacity)), next_seq: AtomicU64::new(1), capacity }
    }

    /// Push a log line. Attaches an atomic monotonic sequence number.
    pub fn push(&self, line: String) {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let mut guard = self.entries.write().unwrap();
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back((seq, line));
    }

    /// Return the last `count` lines in insertion order.
    pub fn snapshot(&self, count: usize) -> Vec<String> {
        let guard = self.entries.read().unwrap();
        let start = guard.len().saturating_sub(count);
        guard.iter().skip(start).map(|(_, line)| line.clone()).collect()
    }

    /// Return the last `count` (seq, line) pairs in insertion order.
    pub fn snapshot_with_seq(&self, count: usize) -> Vec<(u64, String)> {
        let guard = self.entries.read().unwrap();
        let start = guard.len().saturating_sub(count);
        guard.iter().skip(start).map(|(seq, line)| (*seq, line.clone())).collect()
    }
}

pub static LOG_RING: Lazy<RwLock<LogRingBuffers>> = Lazy::new(|| RwLock::new(LogRingBuffers::new(LOG_RING_CAPACITY)));
static MINION_BINARY_PATH: Lazy<Option<String>> = Lazy::new(|| std::env::current_exe().ok().map(|path| path.display().to_string()));
static MINION_BINARY_SHA256: Lazy<Option<String>> =
    Lazy::new(|| std::env::current_exe().ok().and_then(|path| util::iofs::get_file_sha256(path).ok()));

#[derive(Debug, Clone, Copy, Default)]
struct BacklogSnapshot {
    journal_cycles: usize,
    journal_entries: usize,
    journal_bytes: u64,
    dpq_pending: usize,
    dpq_inflight: usize,
}

impl BacklogSnapshot {
    fn format(self) -> String {
        format!(
            "journal={}/{} entries ({} bytes), dpq={}/{} pending/inflight",
            self.journal_cycles, self.journal_entries, self.journal_bytes, self.dpq_pending, self.dpq_inflight
        )
    }
}

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());

fn minion_traits(cfg: &MinionConfig, q: bool, include_binary_sha: bool) -> SystemTraits {
    let mut traits = if q { traits::get_minion_traits_nolog(Some(cfg)) } else { traits::get_minion_traits(Some(cfg)) };
    traits.put("minion.version".to_string(), json!(env!("CARGO_PKG_VERSION")));
    if let Some(path) = &*MINION_BINARY_PATH {
        traits.put("minion.binary.path".to_string(), json!(path));
    }
    if include_binary_sha && let Some(sha256) = &*MINION_BINARY_SHA256 {
        traits.put("minion.binary.sha256".to_string(), json!(sha256));
    }
    traits
}

fn matches_target(cmd: &MasterMessage, minion_id: &str, traits: &SystemTraits) -> bool {
    let tgt = cmd.target();
    if !tgt.id().is_empty() {
        return tgt.id().eq(minion_id);
    }

    if !tgt.hostnames().is_empty()
        && !tgt.hostnames().into_iter().any(|pattern| {
            glob::Pattern::new(&pattern).ok().is_some_and(|pattern| {
                [
                    dataconv::as_str(traits.get("system.hostname.fqdn")),
                    dataconv::as_str(traits.get("system.hostname")),
                    dataconv::as_str(traits.get("system.hostname.ip")),
                ]
                .into_iter()
                .any(|label| !label.is_empty() && pattern.matches(&label))
            })
        })
    {
        return false;
    }

    if !tgt.traits_query().is_empty() {
        return traits::parse_traits_query(tgt.traits_query())
            .ok()
            .and_then(|query| traits::to_typed_query(query).ok())
            .is_some_and(|query| traits::matches_traits(query, traits.clone()));
    }

    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RotationCommandPayload {
    op: String,
    reason: String,
    grace_seconds: u64,
    reconnect: bool,
    reregister: bool,
    intent: SignedRotationIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RegistrationOutcome {
    #[default]
    Pending,
    Accepted,
    Rejected(String),
}

#[derive(Debug)]
pub struct SysMinion {
    cfg: MinionConfig,
    fingerprint: Option<String>,
    kman: MinionRSAKeyManager,

    rstm: Arc<Mutex<Option<OwnedReadHalf>>>,
    wstm: Arc<Mutex<Option<OwnedWriteHalf>>>,

    filedata: Mutex<MinionFiledata>,

    pub(crate) last_ping: Mutex<Instant>,
    pub(crate) ping_timeout: Duration,

    pt_counter: Mutex<PTCounter>,
    dpq: Arc<DiskPersistentQueue>,
    pub(crate) journal: Journal,
    inbound_cmds: InboundCommandLedger,
    connected: AtomicBool,
    secure: Mutex<Option<SecureChannel>>,

    minion_id: String,
    registration: Mutex<RegistrationOutcome>,

    pub(crate) sensors_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) sensors_pump: Mutex<Option<JoinHandle<()>>>,
    pub(crate) ping_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) proto_task: Mutex<Option<JoinHandle<()>>>,
    pub(crate) stats_task: Mutex<Option<JoinHandle<()>>>,
}

impl SysMinion {
    fn classify_execution_failure(err: &SysinspectError) -> &'static str {
        let msg = err.to_string();

        if msg.contains("Missing module") {
            return "module_resolve";
        }
        if msg.contains("Error loading model DSL") || msg.contains("Cannot resolve model path") {
            return "model_load";
        }
        if msg.contains("Entities \"") && msg.contains("are not bound with the state") {
            return "action_select";
        }
        if msg.contains("Action chain requires definition") || msg.contains("expected to be already") {
            return "action_chain";
        }
        if msg.contains("failed to run") {
            return "action_run";
        }

        "execution"
    }

    fn build_execution_failure_response(cycle_id: &str, scheme: &str, context: &str, phase: &str, error: &str, minion_id: &str) -> ActionResponse {
        let mut response = ActionModResponse::with_retcode(1);
        response.set_message(error.to_string());
        response.set_data(json!({
            "kind": "execution_error",
            "phase": phase,
            "error": error,
            "query": scheme,
            "context": context,
            "minion_id": minion_id,
        }));

        let mut failure = ActionResponse::new(
            scheme.to_string(),
            "execution_error".to_string(),
            "$".to_string(),
            response,
            ConstraintResponse::new("Model execution failed".to_string()),
        );
        failure.set_cid(cycle_id.to_string());
        failure.set_query(scheme.to_string());
        failure
    }

    #[cfg(test)]
    pub(crate) fn classify_execution_failure_for_test(err: &SysinspectError) -> &'static str {
        Self::classify_execution_failure(err)
    }

    #[cfg(test)]
    pub(crate) fn build_execution_failure_response_for_test(
        cycle_id: &str, scheme: &str, context: &str, phase: &str, error: &str, minion_id: &str,
    ) -> ActionResponse {
        Self::build_execution_failure_response(cycle_id, scheme, context, phase, error, minion_id)
    }

    fn backlog_snapshot(&self) -> BacklogSnapshot {
        let journal = self.journal.stats().unwrap_or_default();
        let dpq = self.dpq.stats();
        BacklogSnapshot {
            journal_cycles: journal.pending_cycles,
            journal_entries: journal.pending_entries,
            journal_bytes: journal.pending_bytes,
            dpq_pending: dpq.pending_jobs,
            dpq_inflight: dpq.inflight_jobs,
        }
    }

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
        ensure_minion_tree(&cfg)?;

        let (rstm, wstm) = match TcpStream::connect(cfg.master()).await {
            Ok(stream) => {
                log::debug!("Network bound at {}", cfg.master());
                let (rstm, wstm) = stream.into_split();
                (Some(rstm), Some(wstm))
            }
            Err(err) if cfg.offline() == MinionOfflineMode::Independent && fingerprint.is_none() => {
                log::warn!(
                    "Initial transport connect to {} failed in independent mode: {}; starting with transport offline while local work continues",
                    cfg.master(),
                    err
                );
                (None, None)
            }
            Err(err) => return Err(err.into()),
        };

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
            journal: Journal::open_with_policy(cfg.journal_path(), cfg.journal_max_bytes(), cfg.backlog_policy())?,
            inbound_cmds: InboundCommandLedger::open(cfg.inbound_commands_dir())?,
            connected: AtomicBool::new(false),
            secure: Mutex::new(None),
            minion_id: dataconv::as_str(SystemTraits::new(cfg.clone(), true).get(traits::SYS_ID)),
            registration: Mutex::new(RegistrationOutcome::Pending),
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
        ensure_minion_tree(&self.cfg)?;

        // Machine id?
        if !self.cfg.machine_id_path().exists() {
            util::write_machine_id(Some(self.cfg.machine_id_path()))?;
        }
        ensure_master_traits_file(&self.cfg)?;
        if let Err(err) = self.kman.ensure_transport_state(self.get_minion_id()) {
            log::warn!("Unable to refresh local transport state from RSA identity: {err}");
        }

        let mut out: Vec<String> = vec![];
        let minion_traits = minion_traits(&self.cfg, false, false);
        for t in minion_traits.trait_keys() {
            out.push(format!("{}: {}", t.to_owned(), dataconv::to_string(minion_traits.get(&t)).unwrap_or_default()));
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
        let mut systraits = minion_traits(cfg, true, false);
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
    pub(crate) async fn request(&self, msg: Vec<u8>, class: OutboundMessageClass) {
        let _ = self.try_request(msg, class).await;
    }

    pub(crate) async fn try_request(&self, msg: Vec<u8>, class: OutboundMessageClass) -> Result<(), SysinspectError> {
        let payload = match self.secure.lock().await.as_mut().map(|secure| secure.seal_bytes(&msg)).transpose() {
            Ok(Some(msg)) => msg,
            Ok(None) => msg,
            Err(err) => {
                log::error!("Failed to encode secure payload to master: {err}");
                self.maybe_signal_delivery_failure(class);
                return Err(err);
            }
        };

        if let Err(err) = self.write_frame(&payload).await {
            log::warn!("Failed to send message to master: {err}; backlog: {}", self.backlog_snapshot().format());
            self.maybe_signal_delivery_failure(class);
            return Err(err);
        }
        log::trace!("To master: {}", String::from_utf8_lossy(&payload));
        Ok(())
    }

    /// Reliability contract for outbound delivery failure:
    ///
    /// - `follow`: any delivery failure is treated as execution-affecting and
    ///   tears the instance down through reconnect.
    /// - `independent` + `SessionControl`: reconnect the transport/session path,
    ///   because the control plane is broken.
    /// - `independent` + `DurableData`: do not stop execution. The payload is
    ///   already durable or replayable, so delivery failure degrades transport
    ///   state but must not collapse local work.
    fn maybe_signal_delivery_failure(&self, class: OutboundMessageClass) {
        match (self.cfg.offline(), class) {
            (MinionOfflineMode::Follow, _) => {
                log::warn!("Follow mode: delivery failure triggers reconnect");
                let _ = CONNECTION_TX.send(());
            }
            (MinionOfflineMode::Independent, OutboundMessageClass::SessionControl) => {
                log::warn!("Independent mode: session/control delivery failed; triggering reconnect");
                let _ = CONNECTION_TX.send(());
            }
            (MinionOfflineMode::Independent, OutboundMessageClass::DurableData) => {
                log::warn!("Independent mode: durable data delivery failed; execution continues, transport degraded");
            }
        }
    }

    /// Write one length-prefixed transport frame to the master connection.
    async fn write_frame(&self, frame: &[u8]) -> Result<(), SysinspectError> {
        let mut stm = self.wstm.lock().await;
        let stm = stm.as_mut().ok_or_else(|| SysinspectError::ProtoError("Transport write half is not connected".to_string()))?;
        stm.write_all(&(frame.len() as u32).to_be_bytes()).await?;
        stm.write_all(frame).await?;
        stm.flush().await?;
        Ok(())
    }

    /// Read one length-prefixed transport frame from the master connection.
    async fn read_frame(&self) -> Result<Vec<u8>, SysinspectError> {
        let mut stm = self.rstm.lock().await;
        let stm = stm.as_mut().ok_or_else(|| SysinspectError::ProtoError("Transport read half is not connected".to_string()))?;
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

    /// Replace the transport streams with a new TCP connection.
    /// Old streams are dropped when the Option values are replaced.
    async fn set_streams(&self, rstm: OwnedReadHalf, wstm: OwnedWriteHalf) {
        *self.rstm.lock().await = Some(rstm);
        *self.wstm.lock().await = Some(wstm);
    }

    /// Disconnect transport streams (take them out of the Option).
    pub(crate) async fn clear_streams(&self) {
        *self.rstm.lock().await = None;
        *self.wstm.lock().await = None;
    }

    /// Re-establish the full transport stack in `independent` mode without
    /// destroying the execution runtime.
    ///
    /// Contract:
    /// - this path only repairs transport/session state
    /// - journal replay is NOT fired here blindly
    /// - replay is deferred until the master resumes logical session flow by
    ///   requesting fresh `Traits`
    /// - therefore local execution and durable-delivery recovery stay decoupled
    pub(crate) async fn reconnect_transport(self: &Arc<Self>) -> Result<(), SysinspectError> {
        log::info!("Re-establishing transport to master {}...", self.cfg.master());

        // 1. Stop proto, ping, and stats tasks so they don't fight for the old streams.
        Arc::clone(self).stop_background().await;

        // 2. Drop old streams, then open a fresh connection.
        self.clear_streams().await;
        let (rstm, wstm) = match TcpStream::connect(self.cfg.master()).await {
            Ok(s) => s.into_split(),
            Err(err) => {
                log::error!("Failed to connect to master during transport recovery: {err}");
                return Err(SysinspectError::MinionGeneralError(format!("Transport recovery connect failed: {err}")));
            }
        };
        self.set_streams(rstm, wstm).await;
        log::info!("Transport socket reconnected to {}", self.cfg.master());

        // 3. Bootstrap a fresh secure session.
        *self.secure.lock().await = None;
        if let Err(err) = self.bootstrap_secure().await {
            log::error!("Secure bootstrap failed during transport recovery: {err}");
            self.clear_streams().await;
            return Err(err);
        }

        // 4. Restart the proto read loop and background tasks.
        Arc::clone(self).do_proto().await?;

        // 5. Re-establish session with the master. Journal replay is deferred
        // until the master asks for fresh Traits, which confirms the logical
        // session is live again.
        Arc::clone(self).send_ehlo().await?;
        // Resync sensors after reconnection.
        if let Err(err) = Arc::clone(self).send_sensors_sync().await {
            log::warn!("Sensors sync after transport recovery failed: {err}");
        }

        let state = Arc::new(ExitState::new());
        Arc::clone(self).do_ping_update(state).await?;
        Arc::clone(self).do_stats_update().await?;

        log::info!("Transport recovery complete; backlog: {}", self.backlog_snapshot().format());
        Ok(())
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
                log::error!("Unable to prepare secure bootstrap for master {}: {}", self.cfg.master(), err);
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
                        log::error!(
                            "Secure bootstrap ack verification failed for master {} using key {}: {}",
                            self.cfg.master(),
                            opening_key_id,
                            err
                        );
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
                log::error!(
                    "Master {} rejected secure bootstrap with {:?}: {} (retryable={}, rate_limit={})",
                    self.cfg.master(),
                    diag.code,
                    diag.message,
                    diag.failure.retryable,
                    diag.failure.rate_limit
                );
                self.mark_broken_transport(&store, &mut state, Some(&opening_key_id));
                Err(SysinspectError::ProtoError(format!("Master rejected secure bootstrap with {:?}: {}", diag.code, diag.message)))
            }
            _ => {
                log::error!("Master {} replied with a non-bootstrap frame during secure bootstrap", self.cfg.master());
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
                        match this.cfg.offline() {
                            MinionOfflineMode::Follow => {
                                log::warn!(
                                    "Master seems unresponsive in follow mode; reconnect teardown will stop execution. Backlog: {}",
                                    this.backlog_snapshot().format()
                                );
                                state.exit.store(true, Ordering::Relaxed);
                            }
                            MinionOfflineMode::Independent => {
                                log::warn!(
                                    "Master seems unresponsive in independent mode; marking transport unavailable and starting background recovery while execution continues. Backlog: {}",
                                    this.backlog_snapshot().format()
                                );
                            }
                        }
                        let _ = reconnect_sender.send(());
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
            let mut service = SensorService::new(spec).with_ctx(SensorCtx::default().with_sharelib_root(self.cfg.sharelib_dir()));
            service.set_event_processor(events.clone());
            service.spawn()
        };
        *self.sensors_task.lock().await = Some(sensors_task);

        Ok(())
    }

    pub async fn do_stats_update(self: Arc<Self>) -> Result<(), SysinspectError> {
        let this = self.clone();
        let handle = tokio::spawn(async move {
            let mut last_warn_level: u8 = 0;
            let mut last_dpq_warn_level: u8 = 0;
            loop {
                sleep(Duration::from_secs(5)).await;
                let mut ptc = this.pt_counter.lock().await;
                ptc.update_stats();
                drop(ptc);

                let snap = this.backlog_snapshot();

                // Periodically inspect backlog growth so operators see it
                // before the journal budget forces eviction.
                let max_bytes = this.cfg.journal_max_bytes();
                if max_bytes > 0 {
                    let ratio = if snap.journal_bytes >= max_bytes { 1.0 } else { snap.journal_bytes as f64 / max_bytes as f64 };

                    let warn_level = if ratio >= 1.0 {
                        3
                    } else if ratio >= 0.9 {
                        2
                    } else if ratio >= 0.75 {
                        1
                    } else {
                        0
                    };

                    if warn_level != last_warn_level {
                        match warn_level {
                            0 => {}
                            1 => log::warn!(
                                "Journal backlog at {:.0}% of budget ({} / {}); delivery is degraded. {}",
                                ratio * 100.0,
                                snap.journal_bytes,
                                max_bytes,
                                snap.format()
                            ),
                            2 => log::warn!(
                                "Journal backlog at {:.0}% of budget ({} / {}); eviction is imminent. {}",
                                ratio * 100.0,
                                snap.journal_bytes,
                                max_bytes,
                                snap.format()
                            ),
                            _ => log::error!(
                                "Journal backlog has exceeded budget ({} > {}); oldest cycle may be evicted. {}",
                                snap.journal_bytes,
                                max_bytes,
                                snap.format()
                            ),
                        }
                        last_warn_level = warn_level;
                    }
                }

                let dpq_total = snap.dpq_pending + snap.dpq_inflight;
                let dpq_warn_level = if dpq_total >= 100 {
                    3
                } else if dpq_total >= 25 {
                    2
                } else if dpq_total > 0 {
                    1
                } else {
                    0
                };

                if dpq_warn_level != last_dpq_warn_level {
                    match dpq_warn_level {
                        0 => {}
                        1 => log::warn!("DPQ backlog is non-empty while transport is degraded; {}", snap.format()),
                        2 => log::warn!("DPQ backlog has grown materially while transport is degraded; {}", snap.format()),
                        _ => log::error!(
                            "DPQ backlog is large while transport is degraded; local work is outrunning delivery recovery. {}",
                            snap.format()
                        ),
                    }
                    last_dpq_warn_level = dpq_warn_level;
                }
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
                        match this.cfg.offline() {
                            MinionOfflineMode::Follow => {
                                log::warn!(
                                    "Proto read failed in follow mode: {err}; reconnect teardown will stop execution; backlog: {}",
                                    this.backlog_snapshot().format()
                                );
                            }
                            MinionOfflineMode::Independent => {
                                log::warn!(
                                    "Proto read failed in independent mode: {err}; marking transport unavailable and starting background recovery while execution continues; backlog: {}",
                                    this.backlog_snapshot().format()
                                );
                            }
                        }
                        let _ = CONNECTION_TX.send(());
                        break;
                    }
                };

                let msg = match this.secure.lock().await.as_mut().map(|secure| secure.open_bytes(&msg)).transpose() {
                    Ok(Some(msg)) => msg,
                    Ok(None) => msg,
                    Err(err) => {
                        match this.cfg.offline() {
                            MinionOfflineMode::Follow => {
                                log::error!(
                                    "Failed to decode secure frame from master in follow mode: {err}; reconnect teardown will stop execution"
                                );
                            }
                            MinionOfflineMode::Independent => {
                                log::error!(
                                    "Failed to decode secure frame from master in independent mode: {err}; marking transport unavailable and starting background recovery while execution continues"
                                );
                            }
                        }
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
                        if let Some(pinned) = this.fingerprint.as_deref() {
                            match serde_json::from_value::<RegistrationReply>(msg.payload().clone()) {
                                Ok(reply) if !reply.accepted_flag() => {
                                    log::error!("Registration rejected: {}", reply.message());
                                    *this.registration.lock().await = RegistrationOutcome::Rejected(reply.message().to_string());
                                }
                                Ok(reply) => {
                                    let master_pem = match reply.master_key_pem() {
                                        Some(pem) if !pem.trim().is_empty() => pem,
                                        _ => {
                                            let err = "Registration reply did not include the master public key".to_string();
                                            log::error!("{err}");
                                            *this.registration.lock().await = RegistrationOutcome::Rejected(err);
                                            let _ = CONNECTION_TX.send(());
                                            break;
                                        }
                                    };
                                    let master_fp = match reply.master_fingerprint() {
                                        Some(fp) if !fp.trim().is_empty() => fp,
                                        _ => {
                                            let err = "Registration reply did not include the master fingerprint".to_string();
                                            log::error!("{err}");
                                            *this.registration.lock().await = RegistrationOutcome::Rejected(err);
                                            let _ = CONNECTION_TX.send(());
                                            break;
                                        }
                                    };
                                    if master_fp.trim() != pinned.trim() {
                                        let err = format!("Registration fingerprint mismatch: expected {}, got {}", pinned.trim(), master_fp.trim());
                                        log::error!("{err}");
                                        *this.registration.lock().await = RegistrationOutcome::Rejected(err);
                                    } else {
                                        match this.kman.trust_master_identity(this.get_minion_id(), master_pem, Some(pinned)) {
                                            Ok(fp) => {
                                                log::info!("Trusted master fingerprint: {fp}");
                                                *this.registration.lock().await = RegistrationOutcome::Accepted;
                                            }
                                            Err(err) => {
                                                log::error!("Failed to persist trusted master identity: {err}");
                                                *this.registration.lock().await = RegistrationOutcome::Rejected(err.to_string());
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    let err = format!("Failed to decode registration reply: {err}");
                                    log::error!("{err}");
                                    *this.registration.lock().await = RegistrationOutcome::Rejected(err);
                                }
                            }
                        } else {
                            log::warn!("Master requires reconnection: {}", msg.payload());
                        }
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::Remove => {
                        log::debug!("Master asks to unregister");
                        let _ = CONNECTION_TX.send(());
                        break;
                    }

                    RequestType::CycleAck => {
                        let cycle_id = msg.payload().get("cycle_id").and_then(|v| v.as_str()).unwrap_or("?");
                        match this.journal.ack_cycle(cycle_id) {
                            Ok(n) if n > 0 => {
                                log::debug!(
                                    "Journal freed {} entries for cycle {}; remaining backlog: {}",
                                    n,
                                    cycle_id,
                                    this.backlog_snapshot().format()
                                )
                            }
                            Ok(_) => log::debug!("Journal cycle {} already acked", cycle_id),
                            Err(err) => log::error!("Failed to ack journal cycle {}: {}", cycle_id, err),
                        }
                    }

                    RequestType::Command => {
                        log::debug!("Master sends a command");
                        match msg.get_retcode() {
                            ProtoErrorCode::Success => {
                                if msg.target().scheme().starts_with(SCHEME_COMMAND) {
                                    if matches_target(&msg, this.get_minion_id(), &minion_traits(&this.cfg, true, false)) {
                                        this.clone().call_internal_command(msg.cycle(), msg.target().scheme(), msg.target().context()).await;
                                    } else {
                                        log::debug!("Dropped internal master command for another minion");
                                    }
                                } else {
                                    if !matches_target(&msg, this.get_minion_id(), &minion_traits(&this.cfg, true, false)) {
                                        log::debug!("Dropped master model command for another minion");
                                        continue;
                                    }
                                    match this.claim_inbound_command(&msg) {
                                        Ok(InboundCommandClaim::AcceptedNew) => {
                                            if let Err(err) = this.as_ptr().dpq.add(WorkItem::MasterCommand(msg.to_owned())) {
                                                let _ = this.inbound_cmds.remove(&this.inbound_command_replay_key(msg.cycle()));
                                                log::error!("Failed to enqueue master command: {err}");
                                            } else {
                                                log::debug!("Scheduled master command: {}", msg.target().scheme());
                                            }
                                        }
                                        Ok(InboundCommandClaim::Duplicate(state)) => {
                                            log::debug!("Dropped duplicate inbound master command for cycle {} in state {:?}", msg.cycle(), state);
                                            if state == InboundCommandState::Completed {
                                                // Completed duplicates must re-drive ModelAck so the master can
                                                // clear any stale outbound backlog after replay or restart.
                                                this.send_model_ack(msg.cycle()).await;
                                            }
                                        }
                                        Err(err) => {
                                            log::error!("Failed to claim inbound master command {}: {}", msg.cycle(), err);
                                        }
                                    }
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
                                let this = this.clone();
                                tokio::spawn(async move {
                                    this.replay_pending().await;
                                });
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

                                this.request(
                                    proto::msg::get_pong(this.get_minion_id(), ProtoValue::PingTypeGeneral, Some(pl)),
                                    OutboundMessageClass::SessionControl,
                                )
                                .await;
                            }

                            Ok(ProtoValue::PingTypeDiscovery) => {
                                log::debug!("Received discovery ping from master");
                                this.request(
                                    proto::msg::get_pong(this.get_minion_id(), ProtoValue::PingTypeDiscovery, None),
                                    OutboundMessageClass::SessionControl,
                                )
                                .await;
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
        let fresh_traits = minion_traits(&self.cfg, false, true);
        let mut r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Traits, fresh_traits.to_transport_value()?);
        r.set_sid(MINION_SID.to_string());
        self.try_request(
            r.sendable().map_err(|e| {
                log::error!("Error preparing traits message: {e}");
                e
            })?,
            OutboundMessageClass::SessionControl,
        )
        .await?;
        Ok(())
    }

    /// Send ehlo
    pub async fn send_ehlo(self: Arc<Self>) -> Result<(), SysinspectError> {
        let fresh_traits = minion_traits(&self.cfg, false, false);
        let mut r = MinionMessage::new(dataconv::as_str(fresh_traits.get(traits::SYS_ID)), RequestType::Ehlo, fresh_traits.to_json_value()?);
        r.set_sid(MINION_SID.to_string());

        log::info!("Ehlo on {}", self.cfg.master());
        self.try_request(r.sendable()?, OutboundMessageClass::SessionControl).await?;
        Ok(())
    }

    /// Send registration request
    pub async fn send_registration(self: Arc<Self>, pbk_pem: String) -> Result<(), SysinspectError> {
        let r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Add, json!(pbk_pem));

        log::info!("Registration request to {}", self.cfg.master());
        self.try_request(r.sendable()?, OutboundMessageClass::SessionControl).await?;
        Ok(())
    }

    /// Send callback to the master on the results
    pub async fn send_callback(self: Arc<Self>, ar: ActionResponse) -> Result<(), SysinspectError> {
        log::debug!("Sending sync callback on {}", ar.aid());
        log::debug!("Callback: {ar:#?}");
        let msg = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Event, json!(ar)).sendable()?;
        self.journal.append(ar.cid(), &msg)?;
        self.request(msg, OutboundMessageClass::DurableData).await;
        Ok(())
    }

    /// Send finalisation marker callback to the master on the results
    pub async fn send_fin_callback(self: Arc<Self>, ar: ActionResponse) -> Result<(), SysinspectError> {
        log::debug!("Sending fin sync callback on {}", ar.aid());
        let msg = MinionMessage::new(self.get_minion_id().to_string(), RequestType::ModelEvent, json!(ar)).sendable()?;
        self.journal.append(ar.cid(), &msg)?;
        self.request(msg, OutboundMessageClass::DurableData).await;
        Ok(())
    }

    pub(crate) async fn send_sensors_sync(self: Arc<Self>) -> Result<(), SysinspectError> {
        log::info!("Sending sensors sync callback for cycle");
        let mut r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::SensorsSyncRequest, json!({}));
        r.set_sid(MINION_SID.to_string());
        self.try_request(r.sendable()?, OutboundMessageClass::SessionControl).await?;
        Ok(())
    }

    /// Send bye message
    pub async fn send_bye(self: Arc<Self>) {
        let r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Bye, json!(MINION_SID.to_string()));

        log::info!("Goodbye to {}", self.cfg.master());
        match r.sendable() {
            Ok(msg) => self.request(msg, OutboundMessageClass::SessionControl).await,
            Err(e) => log::error!("Failed to send bye message: {e}"),
        }
    }

    /// Send model completion ACK to master.
    async fn send_model_ack(self: &Arc<Self>, cycle_id: &str) {
        let r = MinionMessage::new(self.get_minion_id().to_string(), RequestType::ModelAck, json!({"cycle_id": cycle_id}));
        match r.sendable() {
            Ok(msg) => {
                if let Err(e) = self.journal.append(cycle_id, &msg) {
                    log::error!("Failed to journal model ack for cycle {}: {}", cycle_id, e);
                } else if let Err(e) = self.journal.mark_cycle_locally_complete(cycle_id) {
                    log::error!("Failed to mark cycle {} as locally complete: {}", cycle_id, e);
                } else {
                    // This is the durable local-completion boundary. After this point a hard
                    // restart may replay delivery, but must not re-run the model locally.
                    log::debug!("Marked cycle {} as locally complete after journaling ModelAck", cycle_id);
                }
                if let Err(err) = self.inbound_cmds.set_state(&self.inbound_command_replay_key(cycle_id), InboundCommandState::Completed) {
                    log::error!("Failed to mark inbound command cycle {} as completed: {}", cycle_id, err);
                }
                self.request(msg, OutboundMessageClass::DurableData).await
            }
            Err(e) => log::error!("Failed to send model ack: {e}"),
        }
    }

    async fn send_command_reply(self: &Arc<Self>, cycle_id: &str, payload: Result<serde_json::Value, SysinspectError>) {
        let reply = match payload {
            Ok(payload) => MinionCommandReply { cycle_id: cycle_id.to_string(), ok: true, error: String::new(), payload },
            Err(err) => MinionCommandReply { cycle_id: cycle_id.to_string(), ok: false, error: err.to_string(), payload: serde_json::Value::Null },
        };
        let msg = MinionMessage::new(self.get_minion_id().to_string(), RequestType::Response, json!(reply));
        match msg.sendable() {
            Ok(data) => self.request(data, OutboundMessageClass::SessionControl).await,
            Err(err) => log::error!("Failed to encode command reply for cycle {}: {}", cycle_id, err),
        }
    }

    fn read_log_snapshot(&self, request: &ConsoleMinionLogRequest) -> Result<ConsoleMinionLogSnapshot, SysinspectError> {
        let keep = request.lines.max(1);
        let stdout_path = self.cfg.logfile_std();
        let stderr_path = self.cfg.logfile_err();

        let stdout_file = Self::read_log_lines(&stdout_path)?;
        let stderr_file = Self::read_log_lines(&stderr_path)?;

        let (source_kind, path, lines) = if stdout_file.is_some() || stderr_file.is_some() {
            let mut merged = Vec::new();
            if let Some(lines) = stdout_file {
                merged.extend(lines);
            }
            if let Some(lines) = stderr_file {
                merged.extend(lines);
            }
            merged.retain(|line| Self::keep_operator_log_line(line));
            merged.sort_by_cached_key(|line| Self::log_timestamp_key(line));
            ("file".to_string(), format!("{} + {}", stdout_path.display(), stderr_path.display()), merged)
        } else {
            let ring = LOG_RING.read().unwrap();
            let entries = ring.snapshot_with_seq(keep);
            let mut pairs: Vec<(String, u64, String)> = entries
                .into_iter()
                .map(|(seq, line)| {
                    let ts_key = Self::log_timestamp_str(&line);
                    (ts_key, seq, line)
                })
                .collect();
            pairs.retain(|(_, _, line)| Self::keep_operator_log_line(line));
            pairs.sort_by(|(ta, sa, _), (tb, sb, _)| ta.cmp(tb).then(sa.cmp(sb)));
            let merged = pairs.into_iter().map(|(_, _, line)| line).collect();
            ("in memory".to_string(), "runtime ring".to_string(), merged)
        };

        let start = lines.len().saturating_sub(keep);

        Ok(ConsoleMinionLogSnapshot {
            minion_id: self.get_minion_id().to_string(),
            source_kind,
            path,
            lines: lines[start..].to_vec(),
            truncated: start > 0,
        })
    }

    fn read_log_lines(path: &Path) -> Result<Option<Vec<String>>, SysinspectError> {
        match fs::read(path) {
            Ok(data) => {
                let text = String::from_utf8_lossy(&data);
                Ok(Some(text.lines().map(|line| libsysinspect::logger::strip_ansi_codes(line).into_owned()).collect()))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(SysinspectError::IoErr(std::io::Error::new(err.kind(), format!("{}: {}", path.display(), err)))),
        }
    }

    fn keep_operator_log_line(line: &str) -> bool {
        if let Some((_, rest)) = line.split_once(" - ")
            && let Some((level, _)) = rest.split_once(':')
        {
            return matches!(level.trim(), "INFO" | "WARN" | "WARNING" | "ERROR");
        }
        true
    }

    fn log_timestamp_str(line: &str) -> String {
        Self::log_timestamp_key(line).0
    }

    fn log_timestamp_key(line: &str) -> (String, String) {
        if let Some(end) = line.find(']')
            && line.starts_with('[')
        {
            let ts = &line[1..end];
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%d/%m/%Y %H:%M:%S") {
                return (dt.format("%Y%m%d%H%M%S").to_string(), line.to_string());
            }
        }
        (String::new(), line.to_string())
    }

    fn inbound_command_replay_key(&self, cycle_id: &str) -> String {
        libsysproto::replay::replay_identity_for_master_command_cycle(self.get_minion_id(), cycle_id).key()
    }

    fn claim_inbound_command(&self, msg: &MasterMessage) -> Result<InboundCommandClaim, SysinspectError> {
        self.inbound_cmds.claim(&self.inbound_command_replay_key(msg.cycle()), msg.cycle())
    }

    #[cfg(test)]
    pub(crate) fn claim_inbound_command_for_test(&self, msg: &MasterMessage) -> Result<InboundCommandClaim, SysinspectError> {
        self.claim_inbound_command(msg)
    }

    #[cfg(test)]
    pub(crate) fn inbound_command_state_for_test(&self, cycle_id: &str) -> Result<Option<InboundCommandState>, SysinspectError> {
        self.inbound_cmds.state(&self.inbound_command_replay_key(cycle_id))
    }

    #[cfg(test)]
    pub(crate) fn set_inbound_command_state_for_test(&self, cycle_id: &str, state: InboundCommandState) -> Result<bool, SysinspectError> {
        self.inbound_cmds.set_state(&self.inbound_command_replay_key(cycle_id), state)
    }

    #[cfg(test)]
    pub(crate) async fn has_secure_channel_for_test(&self) -> bool {
        self.secure.lock().await.is_some()
    }

    fn pending_cycle_needs_model_ack(entries: &[(u64, Vec<u8>)]) -> bool {
        let mut has_model_event = false;
        let mut has_model_ack = false;

        for (_, payload) in entries {
            if let Ok(Some(identity)) = replay_identity_from_minion_bytes(payload) {
                match identity {
                    ReplayIdentity::MasterCommand { .. } => {}
                    ReplayIdentity::ModelEvent { .. } => has_model_event = true,
                    ReplayIdentity::ModelAck { .. } => has_model_ack = true,
                    ReplayIdentity::Event { .. } => {}
                }
            }
        }

        has_model_event && !has_model_ack
    }

    /// Replay durable backlog after session recovery.
    ///
    /// Contract:
    /// - replay is at-least-once; duplicates are expected and handled by master
    ///   replay identity + dedup
    /// - replay repairs delivery only; it must not imply local re-execution
    /// - completed cycles are still freed only by master `CycleAck`
    async fn replay_pending(self: Arc<Self>) {
        match self.journal.pending() {
            Ok(cycles) => {
                let before = self.backlog_snapshot();
                let cycle_count = cycles.len();
                let mut total_entries = 0usize;
                for (cycle_id, entries) in &cycles {
                    for (_seq, payload) in entries {
                        if self.try_request(payload.clone(), OutboundMessageClass::DurableData).await.is_ok() {
                            total_entries += 1;
                        }
                    }
                    // Legacy journal entries may predate durable ModelAck journaling. Re-emit a
                    // fresh ModelAck so master can still drive cycle cleanup with `CycleAck`.
                    if Self::pending_cycle_needs_model_ack(entries) {
                        self.send_model_ack(cycle_id).await;
                    }
                }
                if total_entries > 0 {
                    log::info!(
                        "Resent {} journal entries from {} cycle(s) to master; backlog before replay: {}",
                        total_entries,
                        cycle_count,
                        before.format()
                    );
                }
            }
            Err(e) => log::error!("Failed to read pending journal entries: {}", e),
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

    /// Replace the running sysminion binary with a newer build from the master.
    async fn upgrade_self(self: Arc<Self>, request: ConsoleMinionUpgradeSelfRequest, cycle_id: &str) {
        log::info!("Self-upgrade to version {} (checksum {})", request.version.bright_yellow(), request.checksum.yellow());
        let data = match self.clone().download_file(&request.subpath).await {
            Ok(d) => d,
            Err(err) => {
                self.as_ptr().send_command_reply(cycle_id, Err(err)).await;
                return;
            }
        };
        let exe = match std::env::current_exe().and_then(std::fs::canonicalize) {
            Ok(p) => p,
            Err(err) => {
                self.as_ptr().send_command_reply(cycle_id, Err(SysinspectError::IoErr(err))).await;
                return;
            }
        };
        let stage = exe.with_extension("upgrade");
        let backup = exe.with_extension("old");
        if let Err(err) = std::fs::write(&stage, data) {
            self.as_ptr().send_command_reply(cycle_id, Err(SysinspectError::IoErr(err))).await;
            return;
        }
        if let Err(err) = std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o755)) {
            log::warn!("Unable to set upgrade binary permissions: {err}");
        }
        let actual = match util::iofs::get_file_sha256(stage.clone()) {
            Ok(s) => s,
            Err(err) => {
                let _ = std::fs::remove_file(&stage);
                self.as_ptr().send_command_reply(cycle_id, Err(err)).await;
                return;
            }
        };
        if actual != request.checksum {
            let _ = std::fs::remove_file(&stage);
            self.as_ptr()
                .send_command_reply(
                    cycle_id,
                    Err(SysinspectError::MinionGeneralError(format!("Checksum mismatch: expected {}, got {}", request.checksum, actual))),
                )
                .await;
            return;
        }
        let _ = std::fs::remove_file(&backup);
        if let Err(err) = std::fs::rename(&exe, &backup) {
            let _ = std::fs::remove_file(&stage);
            self.as_ptr().send_command_reply(cycle_id, Err(SysinspectError::IoErr(err))).await;
            return;
        }
        if let Err(err) = std::fs::rename(&stage, &exe) {
            let _ = std::fs::rename(&backup, &exe);
            self.as_ptr().send_command_reply(cycle_id, Err(SysinspectError::IoErr(err))).await;
            return;
        }
        self.as_ptr()
            .send_command_reply(cycle_id, Ok(json!({"status": "upgrading", "version": request.version, "checksum": request.checksum})))
            .await;
        log::info!("Restarting sysminion from {} after upgrade", exe.display().to_string().bright_green());
        if let Err(err) = Command::new(&exe).arg("--daemon").spawn() {
            log::error!("Failed to start upgraded sysminion: {err}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        std::process::exit(0);
    }

    /// Download a file from master
    async fn download_file(self: Arc<Self>, fname: &str) -> Result<Vec<u8>, SysinspectError> {
        async fn fetch_file(url: &str, filename: &str) -> Result<Vec<u8>, SysinspectError> {
            let url = format!("http://{}/{}", url.trim_end_matches('/'), filename.to_string().trim_start_matches('/'));
            let rsp = match reqwest::get(url.to_owned()).await {
                Ok(rsp) => rsp,
                Err(err) => {
                    return Err(SysinspectError::MinionGeneralError(format!("Unable to get file at {url}: {err}")));
                }
            };

            match rsp.status() {
                reqwest::StatusCode::OK => match rsp.bytes().await {
                    Ok(data) => Ok(data.to_vec()),
                    Err(err) => Err(SysinspectError::MinionGeneralError(format!("Unable to read bytes from the file: {err}"))),
                },
                reqwest::StatusCode::NOT_FOUND => Err(SysinspectError::MinionGeneralError("File not found".to_string())),
                _ => Err(SysinspectError::MinionGeneralError("Unknown status".to_string())),
            }
        }
        let addr = self.cfg.fileserver();
        let fname = fname.to_string();
        let h = tokio::spawn(async move {
            match fetch_file(&addr, &fname).await {
                Ok(data) => {
                    log::debug!("Filename: {fname} contains {} bytes", data.len());
                    Some(data)
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
        async fn emit_execution_failure(minion: Arc<SysMinion>, cycle_id: &str, scheme: &str, context: &str, phase: &str, error: &str) {
            let failure = SysMinion::build_execution_failure_response(cycle_id, scheme, context, phase, error, minion.get_minion_id());

            if let Err(send_err) = minion.send_callback(failure).await {
                log::error!("Failed to send execution failure callback for cycle {}: {}", cycle_id, send_err);
            }
        }

        // Get the query first
        let mqr = match MinionQuery::new(scheme) {
            Ok(mqr) => mqr,
            Err(err) => {
                log::error!("Query error: {err}");
                emit_execution_failure(self.clone(), cycle_id, scheme, context, "query_parse", &err.to_string()).await;
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
                            emit_execution_failure(
                                self.clone(),
                                cycle_id,
                                scheme,
                                context,
                                "model_sync",
                                &format!("Unable to determine parent directory for {}", dst.display()),
                            )
                            .await;
                            return;
                        }
                    };

                    if !dst_dir.exists() {
                        log::debug!("Creating directory: {}", dst_dir.display());
                        if let Err(err) = fs::create_dir_all(&dst_dir) {
                            log::error!("Unable to create directories for model download: {err}");
                            emit_execution_failure(self.clone(), cycle_id, scheme, context, "model_sync", &err.to_string()).await;
                            return;
                        }
                    }

                    log::debug!("Saving URI {uri_file} as {}", dst_dir.display());
                    if let Err(err) = fs::write(&dst, data) {
                        log::error!("Unable to save downloaded file to {}: {err}", dst.display());
                        emit_execution_failure(self.clone(), cycle_id, scheme, context, "model_sync", &err.to_string()).await;
                        return;
                    }
                    dirty = true;
                }
                Err(err) => {
                    log::error!("Unable to auto-update {uri_file}: {err}");
                    continue;
                }
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
            sr.set_traits(minion_traits(&self.cfg, false, false));
            sr.set_context(context::get_context(context));

            sr.add_action_callback(Box::new(ActionResponseCallback::new(self.as_ptr(), cycle_id, scheme)));
            sr.add_model_callback(Box::new(ModelResponseCallback::new(self.as_ptr(), cycle_id, scheme)));

            match tokio::task::spawn_blocking(move || futures::executor::block_on(sr.start())).await {
                Ok(Ok(())) => {
                    log::debug!("Task {} finished", cycle_id);
                }
                Ok(Err(err)) => {
                    log::error!("Model execution failed for cycle {}: {}", cycle_id, err);
                    let phase = Self::classify_execution_failure(&err);
                    emit_execution_failure(self.clone(), cycle_id, scheme, context, phase, &err.to_string()).await;
                }
                Err(e) => {
                    log::error!("Blocking task crashed: {e}");
                    emit_execution_failure(self.clone(), cycle_id, scheme, context, "runner_crash", &e.to_string()).await;
                }
            };
            self.as_ptr().send_model_ack(cycle_id).await;
            self.as_ptr().pt_counter.lock().await.dec(cycle_id);
        }
    }

    /// Calls internal command
    async fn call_internal_command(self: Arc<Self>, cycle_id: &str, cmd: &str, context: &str) {
        let cmd = cmd.strip_prefix(SCHEME_COMMAND).unwrap_or_default();
        match cmd {
            CLUSTER_SHUTDOWN => {
                log::info!("Requesting minion shutdown from a master");
                self.as_ptr().send_bye().await;
                std::process::exit(0);
            }
            CLUSTER_MINION_SHUTDOWN => {
                log::info!("Requesting per-minion shutdown from the console");
                self.as_ptr().send_command_reply(cycle_id, Ok(json!({"status": "shutting_down"}))).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                self.as_ptr().send_bye().await;
                std::process::exit(0);
            }
            CLUSTER_MINION_RECONNECT => {
                log::info!("Requesting forced reconnect from the console");
                self.as_ptr().send_command_reply(cycle_id, Ok(json!({"status": "reconnecting"}))).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                let _ = CONNECTION_TX.send(());
            }
            CLUSTER_REBOOT => {
                log::warn!("Command \"reboot\" is not implemented yet");
            }
            CLUSTER_RECONNECT => {
                log::info!("Requesting cluster-wide reconnect from the master");
                let _ = CONNECTION_TX.send(());
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
            CLUSTER_MINION_LOGS => {
                let payload = serde_json::from_str::<ConsoleMinionLogRequest>(context)
                    .map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse minion log request: {err}")))
                    .and_then(|request| {
                        self.read_log_snapshot(&request).and_then(|snapshot| {
                            serde_json::to_value(snapshot)
                                .map_err(|err| SysinspectError::SerializationError(format!("Failed to encode minion log snapshot: {err}")))
                        })
                    });
                self.as_ptr().send_command_reply(cycle_id, payload).await;
            }
            CLUSTER_MINION_TOP => {
                let minion_id = self.get_minion_id();
                let payload = serde_json::from_str::<ConsoleMinionTopRequest>(context)
                    .map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse minion top request: {err}")))
                    .and_then(|request| {
                        serde_json::to_value(collect_top_snapshot(minion_id, &request))
                            .map_err(|err| SysinspectError::SerializationError(format!("Failed to encode minion top snapshot: {err}")))
                    });
                self.as_ptr().send_command_reply(cycle_id, payload).await;
            }
            CLUSTER_MINION_PROCESS_SIGNAL => {
                let payload = serde_json::from_str::<ConsoleMinionProcessSignalRequest>(context)
                    .map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse minion process signal request: {err}")))
                    .and_then(|request| {
                        if request.pid == 0 {
                            return Err(SysinspectError::InvalidQuery("Process signal pid must be greater than zero".to_string()));
                        }
                        if request.signal <= 0 {
                            return Err(SysinspectError::InvalidQuery("Process signal value must be greater than zero".to_string()));
                        }
                        // SAFETY: libc::kill is called with the caller-provided pid and signal values after basic validation.
                        let rc = unsafe { libc::kill(request.pid as libc::pid_t, request.signal) };
                        if rc == 0 {
                            Ok(json!({"pid": request.pid, "signal": request.signal, "status": "sent"}))
                        } else {
                            Err(SysinspectError::MasterGeneralError(format!(
                                "Failed to send signal {} to pid {}: {}",
                                request.signal,
                                request.pid,
                                std::io::Error::last_os_error()
                            )))
                        }
                    });
                self.as_ptr().send_command_reply(cycle_id, payload).await;
            }
            CLUSTER_MINION_UPGRADE_SELF => match serde_json::from_str::<ConsoleMinionUpgradeSelfRequest>(context) {
                Ok(request) => {
                    self.clone().upgrade_self(request, cycle_id).await;
                }
                Err(err) => {
                    self.as_ptr()
                        .send_command_reply(
                            cycle_id,
                            Err(SysinspectError::DeserializationError(format!("Failed to parse self-upgrade request: {err}"))),
                        )
                        .await;
                }
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
        if !matches_target(&cmd, self.get_minion_id(), &minion_traits(&self.cfg, true, false)) {
            log::debug!("Command was dropped as it targets another minion");
            return;
        }

        log::debug!("Through. {:?}", cmd.payload());
        self.as_ptr().pt_counter.lock().await.inc(cmd.cycle());

        match PayloadType::try_from(cmd.payload().clone()) {
            Ok(PayloadType::ModelOrStatement(pld)) => {
                if cmd.target().scheme().starts_with(SCHEME_COMMAND) {
                    self.as_ptr().call_internal_command(cmd.cycle(), cmd.target().scheme(), cmd.target().context()).await;
                } else {
                    if let Err(err) = self.inbound_cmds.set_state(&self.inbound_command_replay_key(cmd.cycle()), InboundCommandState::Running) {
                        log::error!("Failed to mark inbound command cycle {} as running: {}", cmd.cycle(), err);
                    }
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
                        if !cmd.cycle().is_empty() {
                            match m.journal.is_cycle_locally_complete(cmd.cycle()) {
                                Ok(true) => {
                                    // DPQ recovers unfinished scheduler state after restart. Once a cycle crossed
                                    // the durable local-completion boundary, execution must not happen again here.
                                    log::info!("Skipping already-completed local cycle {}; replay recovery will finish delivery", cmd.cycle());
                                    return Ok(());
                                }
                                Ok(false) => {
                                    log::info!("Cycle {} has no durable local-completion marker; executing recovered job", cmd.cycle());
                                }
                                Err(err) => {
                                    log::error!("Failed to inspect local completion state for cycle {}: {}", cmd.cycle(), err);
                                }
                            }
                        }
                        log::info!("Dispatching recovered or queued job for cycle {}", cmd.cycle());
                        m.clone().dispatch(cmd).await;
                        Ok(())
                    }
                }
            }
        }
    });

    async fn wait_for_runner_drain(minion: &Arc<SysMinion>) {
        let start = Instant::now();
        loop {
            if minion.pt_counter.lock().await.is_done() {
                // Let the queue runner finish the ack path for the just-completed job.
                sleep(Duration::from_millis(100)).await;
                break;
            }
            if start.elapsed() >= RUNNER_DRAIN_TIMEOUT {
                log::warn!("Timed out waiting for in-flight minion work to finish; forcing runner shutdown.");
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    async fn recover_transport_startup(minion: &Arc<SysMinion>, cfg: &MinionConfig) -> Result<(), SysinspectError> {
        if !cfg.reconnect() {
            log::warn!("Reconnect is disabled; leaving transport offline while local execution continues.");
            return Ok(());
        }

        let mut attempts = 0u32;
        loop {
            attempts += 1;
            if cfg.reconnect_freq() > 0 && attempts > cfg.reconnect_freq() {
                log::error!(
                    "Startup transport recovery exceeded {} attempt(s); leaving transport offline while local execution continues.",
                    cfg.reconnect_freq()
                );
                return Ok(());
            }

            match minion.as_ptr().reconnect_transport().await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    let interval = cfg.reconnect_interval();
                    log::error!("Startup transport recovery failed: {err}; retrying in {interval} seconds...");
                    sleep(Duration::from_secs(interval)).await;
                }
            }
        }
    }

    async fn stop_instance(minion: &Arc<SysMinion>, runner: tokio::task::JoinHandle<()>) -> Result<(), tokio::task::JoinError> {
        minion.as_ptr().stop_sensors().await;
        minion.as_ptr().stop_background().await;
        wait_for_runner_drain(minion).await;
        runner.abort();
        runner.await
    }

    // Messages
    if minion.fingerprint.is_some() {
        minion.as_ptr().do_proto().await?;
        minion.as_ptr().send_registration(minion.kman.get_pubkey_pem()).await?;
        match reconnect_rx.recv().await {
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                runner.abort();
                let _ = runner.await;
                return Err(SysinspectError::ProtoError(format!("Missed {n} reconnect notification(s) while waiting for registration")));
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                runner.abort();
                let _ = runner.await;
                return Err(SysinspectError::ProtoError("Reconnect channel closed while waiting for registration".to_string()));
            }
        }
        minion.as_ptr().stop_background().await;
        runner.abort();
        let _ = runner.await;
        return match &*minion.registration.lock().await {
            RegistrationOutcome::Accepted => Ok(()),
            RegistrationOutcome::Rejected(err) => Err(SysinspectError::ProtoError(err.to_string())),
            RegistrationOutcome::Pending => Err(SysinspectError::ProtoError("Registration ended without a trust decision".to_string())),
        };
    } else {
        if let Err(err) = minion.bootstrap_secure().await {
            if cfg.offline() == MinionOfflineMode::Independent {
                log::error!("Unable to bootstrap secure transport at startup: {err}; continuing local work and recovering transport in background");
                let _ = recover_transport_startup(&minion, &cfg).await;
            } else {
                log::error!("Unable to bootstrap secure transport: {err}");
                let _ = stop_instance(&minion, runner).await;
                return Err(err);
            }
        }
        if minion.secure.lock().await.is_some() {
            minion.as_ptr().do_proto().await?;
            minion.as_ptr().send_ehlo().await?;
        }
        if minion.secure.lock().await.is_some() && cfg.autosync_startup() {
            tokio::select! {
                sync_res = modpak.sync() => {
                    if let Err(err) = sync_res {
                        let _ = stop_instance(&minion, runner).await;
                        return Err(err);
                    }
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
        } else if minion.secure.lock().await.is_some() {
            log::warn!("Module auto-sync {} is disabled. Call cluster sync to force modules sync.", "on startup".bright_yellow());
        }
    }

    if state.exit.load(Ordering::Relaxed) {
        wait_for_runner_drain(&minion).await;
        runner.abort();
        let _ = runner.await;
        return Ok(());
    }

    // Send sensors sync request
    if minion.secure.lock().await.is_some() {
        minion.as_ptr().send_sensors_sync().await?;
        minion.as_ptr().do_ping_update(state.clone()).await?;
        minion.as_ptr().do_stats_update().await?;
    }

    // Keeps client running
    while !state.exit.load(std::sync::atomic::Ordering::Relaxed) {
        tokio::select! {
            sig = reconnect_rx.recv() => {
                let exit = match &sig {
                    Ok(_) => {
                        match cfg.offline() {
                            MinionOfflineMode::Independent => {
                                log::warn!("Transport lost in independent mode; starting background recovery");
                                if !cfg.reconnect() {
                                    log::warn!("Reconnect is disabled; leaving transport offline while local execution continues.");
                                    false
                                } else {
                                    let mut attempts = 0u32;
                                    loop {
                                        attempts += 1;
                                        if cfg.reconnect_freq() > 0 && attempts > cfg.reconnect_freq() {
                                            log::error!(
                                                "Transport recovery exceeded {} attempt(s); leaving transport offline while local execution continues.",
                                                cfg.reconnect_freq()
                                            );
                                            break false;
                                        }

                                    match minion.as_ptr().reconnect_transport().await {
                                        Ok(()) => {
                                            reconnect_rx = CONNECTION_TX.subscribe();
                                            break false;
                                        }
                                        Err(err) => {
                                            let interval = cfg.reconnect_interval();
                                            log::error!("Transport recovery failed: {err}; retrying in {interval} seconds...");
                                            sleep(Duration::from_secs(interval)).await;
                                        }
                                    }
                                }
                                }
                            }
                            MinionOfflineMode::Follow => true,
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Missed {n} reconnect notification(s) in main loop; exiting minion instance.");
                        true
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Reconnect channel closed in main loop; exiting minion instance.");
                        true
                    }
                };
                state.exit.store(exit, Ordering::Relaxed);
            }
            _ = sleep(tokio::time::Duration::from_millis(200)) => {}
        }
    }

    minion.as_ptr().stop_sensors().await;
    minion.as_ptr().stop_background().await;

    wait_for_runner_drain(&minion).await;
    runner.abort();
    let _ = runner.await;

    Ok(())
}

pub async fn minion(cfg: MinionConfig, fp: Option<String>) {
    let mut reconnect_rx = CONNECTION_TX.subscribe();
    let mut fp = fp;
    let mut ra = 0;
    let dpq = match DiskPersistentQueue::open(cfg.root_dir().join(CFG_PENDING_TASKS_ROOT)) {
        Ok(dpq) => Arc::new(dpq),
        Err(e) => {
            log::error!("Failed to open disk persistent queue: {e}");
            log::error!(
                "Is there another minion running? If not, delete the {} directory and try again.",
                cfg.root_dir().join(CFG_PENDING_TASKS_ROOT).to_str().unwrap_or_default().bright_yellow()
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

        if cfg.offline() == MinionOfflineMode::Independent {
            match (&mut mhdl).await {
                Ok(Ok(_)) => log::info!("Minion instance ended gracefully, reconnecting..."),
                Ok(Err(e)) => {
                    log::error!("Minion encountered an error: {e:?}");
                    if fp.is_some() {
                        return;
                    }
                }
                Err(e) => log::error!("Minion task panicked or was cancelled: {e:?}"),
            }
        } else {
            tokio::select! {
                res = &mut mhdl => {
                    match res {
                        Ok(Ok(_)) => log::info!("Minion instance ended gracefully, reconnecting..."),
                        Ok(Err(e)) => {
                            log::error!("Minion encountered an error: {e:?}");
                            if fp.is_some() {
                                return;
                            }
                        }
                        Err(e) => log::error!("Minion task panicked or was cancelled: {e:?}"),
                    }
                }
                sig = reconnect_rx.recv() => {
                    match sig {
                        Ok(_) => {
                            log::warn!("Reconnect signal received; waiting for current minion instance to stop.");
                            let _ = mhdl.await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("Missed {n} reconnect notification(s) in supervisor loop; waiting for current minion instance to stop.");
                            let _ = mhdl.await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            log::warn!("Reconnect channel closed in supervisor loop; waiting for minion instance task.");
                            let _ = mhdl.await;
                        }
                    }
                }
            }
        }

        if fp.is_some()
            && cfg.root_dir().join(CFG_MASTER_KEY_PUB).exists()
            && TransportStore::for_minion(&cfg).ok().and_then(|store| store.load().ok().flatten()).is_some()
        {
            log::info!("Registration trust is seeded; switching to secure startup.");
            fp = None;
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
        log::info!("Reconnecting in {interval} seconds... Current backlog: {}", dpq.stats().format());
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
        let (ip, port) = setup_master_addr(args.get_one::<String>("master-addr").map(String::as_str), get_ssh_client_ip())?;
        cfg.set_master_ip(&ip);
        cfg.set_master_port(port);

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

pub(crate) fn setup_master_addr(addr: Option<&str>, ssh_ip: Option<String>) -> Result<(String, u32), SysinspectError> {
    let addr = addr
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .or(ssh_ip)
        .ok_or_else(|| SysinspectError::ConfigError("Master address is missing".to_string()))?;

    Ok(match addr.strip_prefix('[').and_then(|v| v.split_once(']')) {
        Some((host, "")) => (host.to_string(), DEFAULT_PORT),
        Some((host, rest)) => (
            host.to_string(),
            rest.strip_prefix(':')
                .ok_or_else(|| SysinspectError::ConfigError(format!("Invalid master address: {addr}")))?
                .parse::<u32>()
                .map_err(|_| SysinspectError::ConfigError(format!("Invalid master address: {addr}")))?,
        ),
        None if addr.matches(':').count() == 1 => {
            let (host, port) = addr.rsplit_once(':').unwrap_or_default();
            (host.to_string(), port.parse::<u32>().map_err(|_| SysinspectError::ConfigError(format!("Invalid master address: {addr}")))?)
        }
        None => (addr, DEFAULT_PORT),
    })
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
