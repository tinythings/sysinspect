use crate::{
    cluster::VirtualMinionsCluster,
    dataserv::fls,
    registry::{
        mkb::MinionsKeyRegistry,
        mreg::MinionRegistry,
        session::{self, SessionKeeper},
        taskreg::TaskRegistry,
    },
    telemetry::{otel::OtelLogger, rds::FunctionReducer},
    transport::{IncomingFrame, OutgoingFrame, PeerTransport},
};
use chrono::Utc;
use colored::Colorize;
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use libeventreg::{
    ipcs::DbIPCService,
    kvdb::{EventMinion, EventsRegistry},
};
use libmodpak::SysInspectModPak;
use libsysinspect::{
    cfg::mmconf::{CFG_MODELS_ROOT, MasterConfig},
    console::{
        ConsoleEnvelope, ConsoleQuery, ConsoleResponse, ConsoleSealed, authorised_console_client, ensure_console_keypair, load_master_private_key,
    },
    context::{ProfileConsoleRequest, get_context},
    mdescr::{mspec::MODEL_FILE_EXT, mspecdef::ModelSpec, telemetry::DataExportType},
    rsa::rotation::{RotationActor, RsaTransportRotator, SignedRotationIntent},
    transport::{TransportPeerState, TransportStore},
    util::{self, iofs::scan_files_sha256, pad_visible},
};
use libsysproto::{
    self, MasterMessage, MinionMessage, MinionTarget,
    errcodes::ProtoErrorCode,
    payload::{ModStatePayload, PingData},
    query::{
        SCHEME_COMMAND,
        commands::{CLUSTER_ONLINE_MINIONS, CLUSTER_PROFILE, CLUSTER_REMOVE_MINION, CLUSTER_ROTATE, CLUSTER_TRAITS_UPDATE, CLUSTER_TRANSPORT_STATUS},
    },
    rqtypes::{ProtoKey, ProtoValue, RequestType},
    secure::SECURE_PROTOCOL_VERSION,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration as StdDuration;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Weak},
    vec,
};
use tokio::net::{
    TcpListener,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, sleep};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader},
    time,
};

// Session singleton
pub static SHARED_SESSION: Lazy<Arc<Mutex<SessionKeeper>>> = Lazy::new(|| Arc::new(Mutex::new(SessionKeeper::new(30))));
static MODEL_CACHE: Lazy<Arc<Mutex<HashMap<PathBuf, ModelSpec>>>> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
static MAX_CONSOLE_FRAME_SIZE: usize = 64 * 1024;
const CONSOLE_READ_TIMEOUT: StdDuration = StdDuration::from_secs(5);
const DEFAULT_ROTATION_OVERLAP_SECONDS: u64 = 900;

#[derive(Debug, Clone, Deserialize)]
struct RotationConsoleRequest {
    op: Option<String>,
    reason: Option<String>,
    grace_seconds: Option<u64>,
    reconnect: Option<bool>,
    reregister: Option<bool>,
}

impl RotationConsoleRequest {
    fn from_context(context: &str) -> Result<Self, SysinspectError> {
        if context.trim().is_empty() {
            return Ok(Self {
                op: Some("rotate".to_string()),
                reason: Some("manual".to_string()),
                grace_seconds: Some(DEFAULT_ROTATION_OVERLAP_SECONDS),
                reconnect: Some(true),
                reregister: Some(true),
            });
        }
        serde_json::from_str(context).map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse rotate request context: {err}")))
    }

    fn reason(&self) -> &str {
        self.reason.as_deref().unwrap_or("manual")
    }

    fn grace_seconds(&self) -> u64 {
        self.grace_seconds.unwrap_or(DEFAULT_ROTATION_OVERLAP_SECONDS)
    }

    fn reconnect(&self) -> bool {
        self.reconnect.unwrap_or(true)
    }

    fn reregister(&self) -> bool {
        self.reregister.unwrap_or(true)
    }

    fn op(&self) -> &str {
        self.op.as_deref().unwrap_or("rotate")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RotationCommandPayload {
    pub(crate) op: String,
    pub(crate) reason: String,
    pub(crate) grace_seconds: u64,
    pub(crate) reconnect: bool,
    pub(crate) reregister: bool,
    pub(crate) intent: SignedRotationIntent,
}

type ConsoleOutcome = (ConsoleResponse, Vec<MasterMessage>);

/// Summary of how many rotation commands were dispatched immediately versus staged for offline peers.
#[derive(Debug, Default)]
struct RotationDispatchSummary {
    online_count: usize,
    queued_count: usize,
}

impl RotationDispatchSummary {
    /// Record one successful online rotation dispatch.
    fn note_online_dispatch(&mut self) {
        self.online_count += 1;
    }

    /// Record one offline peer whose rotation was queued for later reconnect.
    fn note_queued_dispatch(&mut self) {
        self.queued_count += 1;
    }

    /// Render the operator-facing console response for the staged rotation summary.
    fn response(&self) -> ConsoleResponse {
        ConsoleResponse {
            ok: true,
            message: format!(
                "Rotation staged: {} online dispatch{}, {} pending for offline minion{}",
                self.online_count,
                if self.online_count == 1 { "" } else { "es" },
                self.queued_count,
                if self.queued_count == 1 { "" } else { "s" }
            ),
        }
    }
}

/// One rendered row of transport status data shown in the console summary.
#[derive(Debug)]
struct TransportStatusRow {
    minion_id: String,
    active_key: String,
    key_age: String,
    last_handshake: String,
    rotation: String,
    last_rotated: String,
}

impl TransportStatusRow {
    /// Return the fixed header rows used by the transport status table.
    fn header() -> [String; 2] {
        [
            "MINION  ACTIVE_KEY  KEY_AGE  LAST_HANDSHAKE  ROTATION  security.transport.last-rotated-at".to_string(),
            "------  ----------  -------  --------------  --------  ----------------------------------".to_string(),
        ]
    }

    /// Build the fallback row for a minion with no managed transport state.
    fn missing(minion_id: &str) -> Self {
        Self {
            minion_id: minion_id.to_string(),
            active_key: "-".to_string(),
            key_age: "-".to_string(),
            last_handshake: "-".to_string(),
            rotation: "missing".to_string(),
            last_rotated: "-".to_string(),
        }
    }

    /// Build one transport status row from persisted managed transport state.
    fn from_state(minion_id: &str, state: &TransportPeerState, now: chrono::DateTime<Utc>) -> Self {
        let active_key = state.active_key_id.clone().unwrap_or_else(|| "-".to_string());
        let mut key_age = "-".to_string();
        let mut last_rotated = "-".to_string();
        if let Some(record) = state.keys.iter().find(|key| key.key_id == active_key) {
            let base = record.activated_at.unwrap_or(record.created_at);
            key_age = format!("{}s", (now - base).num_seconds().max(0));
            last_rotated = base.to_rfc3339();
        }

        Self {
            minion_id: minion_id.to_string(),
            active_key,
            key_age,
            last_handshake: state.last_handshake_at.map(|d| d.to_rfc3339()).unwrap_or_else(|| "-".to_string()),
            rotation: format!("{:?}", state.rotation),
            last_rotated,
        }
    }

    /// Render this row for the console table.
    fn render(&self) -> String {
        format!("{}  {}  {}  {}  {}  {}", self.minion_id, self.active_key, self.key_age, self.last_handshake, self.rotation, self.last_rotated)
    }
}

#[derive(Debug)]
pub struct SysMaster {
    cfg: MasterConfig,
    broadcast: broadcast::Sender<MasterMessage>,
    mkr: MinionsKeyRegistry,
    mreg: Arc<Mutex<MinionRegistry>>,
    taskreg: Arc<Mutex<TaskRegistry>>,
    evtipc: Arc<DbIPCService>,
    to_drop: HashSet<String>,
    session: Arc<Mutex<session::SessionKeeper>>,
    ptr: Option<Weak<Mutex<SysMaster>>>,
    vmcluster: VirtualMinionsCluster,
    conn_to_mid: HashMap<String, String>, // Map connection addresses to minion IDs
    peer_transport: PeerTransport,
    datastore: Arc<Mutex<DataStorage>>,
}

impl SysMaster {
    pub fn new(cfg: MasterConfig) -> Result<SysMaster, SysinspectError> {
        let _ = crate::util::log_sensors_export(&cfg, true);

        let (tx, _) = broadcast::channel::<MasterMessage>(100);
        let mkr = MinionsKeyRegistry::new(cfg.minion_keys_root())?;
        let mreg = Arc::new(Mutex::new(MinionRegistry::new(cfg.minion_registry_root())?));
        let taskreg = Arc::new(Mutex::new(TaskRegistry::new()));
        let evtreg = Arc::new(Mutex::new(EventsRegistry::new(cfg.telemetry_location(), cfg.history())?));
        let evtipc = Arc::new(DbIPCService::new(Arc::clone(&evtreg), cfg.telemetry_socket().to_str().unwrap_or_default())?);
        let vmcluster = VirtualMinionsCluster::new(cfg.cluster().to_owned(), Arc::clone(&mreg), Arc::clone(&SHARED_SESSION), Arc::clone(&taskreg));

        let ds_cfg = DataStorageConfig::new()
            .expiration(StdDuration::from_secs(cfg.datastore_max_age()))
            .max_overall_size(cfg.datastore_max_size())
            .max_item_size(cfg.datastore_item_max_size());
        let ds_path = cfg.datastore_path();

        Ok(SysMaster {
            cfg,
            broadcast: tx,
            mkr,
            to_drop: HashSet::default(),
            session: Arc::clone(&SHARED_SESSION),
            mreg,
            evtipc,
            taskreg,
            ptr: None,
            vmcluster,
            conn_to_mid: HashMap::new(),
            peer_transport: PeerTransport::new(),
            datastore: Arc::new(Mutex::new(DataStorage::new(ds_cfg, ds_path)?)),
        })
    }

    /// Parse minion request
    fn to_request(&self, data: &str) -> Option<MinionMessage> {
        match serde_json::from_str::<MinionMessage>(data) {
            Ok(request) => {
                return Some(request);
            }
            Err(err) => {
                log::error!("Error parse minion response: {err}");
            }
        }

        None
    }

    /// Build a plaintext bootstrap diagnostic from shared malformed-attempt state kept for framed transport probes.
    pub(crate) fn secure_peer_diag_with_state(
        failures: &mut HashMap<String, (std::time::Instant, u32)>, peer_addr: &str, data: &[u8],
    ) -> Option<Vec<u8>> {
        PeerTransport::bootstrap_diag_with_state(failures, peer_addr, data)
    }

    /// Return a plaintext diagnostic when a minion sends normal protocol traffic before bootstrap.
    pub(crate) fn plaintext_peer_diag(data: &[u8]) -> Option<Vec<u8>> {
        PeerTransport::plaintext_diag(data)
    }

    /// Return whether this connection may receive plaintext broadcast traffic before a secure session exists.
    pub(crate) fn peer_can_receive_broadcast_state(has_secure_session: bool, plaintext_allowed: bool) -> bool {
        PeerTransport::can_receive_broadcast_state(has_secure_session, plaintext_allowed)
    }

    #[cfg(test)]
    pub(crate) fn replay_cache_key_for_test(binding: &libsysproto::secure::SecureSessionBinding) -> String {
        PeerTransport::replay_cache_key(binding)
    }

    #[cfg(test)]
    pub(crate) fn peer_rate_limit_key_for_test(peer_addr: &str) -> String {
        PeerTransport::rate_limit_key(peer_addr)
    }

    #[cfg(test)]
    pub(crate) fn reject_bootstrap_replay_with_state_for_test(
        cache: &mut HashMap<String, std::time::Instant>, binding: &libsysproto::secure::SecureSessionBinding, now: std::time::Instant,
    ) -> bool {
        PeerTransport::reject_bootstrap_replay(cache, binding, now)
    }

    #[cfg(test)]
    pub(crate) fn accept_bootstrap_auth_then_replay_for_test(
        cache: &mut HashMap<String, std::time::Instant>, state: &libsysinspect::transport::TransportPeerState,
        hello: &libsysproto::secure::SecureBootstrapHello, master_prk: &rsa::RsaPrivateKey, minion_pbk: &rsa::RsaPublicKey, now: std::time::Instant,
    ) -> Result<libsysproto::secure::SecureFrame, SysinspectError> {
        PeerTransport::accept_bootstrap_auth_then_replay_for_test(cache, state, hello, master_prk, minion_pbk, now)
    }

    /// Start sysmaster
    pub async fn init(&mut self) -> Result<(), SysinspectError> {
        log::info!("Starting master at {}", self.cfg.bind_addr());
        ensure_console_keypair(&self.cfg.root_dir())?;
        std::fs::create_dir_all(self.cfg.console_keys_root()).map_err(SysinspectError::IoErr)?;
        self.vmcluster.init().await?;
        Ok(())
    }

    pub fn cfg(&self) -> MasterConfig {
        self.cfg.to_owned()
    }

    pub fn cfg_ref(&self) -> &MasterConfig {
        &self.cfg
    }

    /// Get broadcast sender for master messages
    pub fn broadcast(&self) -> broadcast::Sender<MasterMessage> {
        self.broadcast.clone()
    }

    /// Get datastore
    pub fn datastore(&self) -> Arc<Mutex<DataStorage>> {
        Arc::clone(&self.datastore)
    }

    pub async fn listener(&self) -> Result<TcpListener, SysinspectError> {
        Ok(TcpListener::bind(self.cfg.bind_addr()).await?)
    }

    /// Return a cloned Arc if ptr is set, else None
    pub fn as_ptr(&self) -> Option<Arc<Mutex<SysMaster>>> {
        self.ptr.as_ref()?.upgrade()
    }

    /// Get Minion key registry
    fn mkr(&mut self) -> &mut MinionsKeyRegistry {
        &mut self.mkr
    }

    /// Decode one raw peer frame through the transport manager using the current master configuration and key registry.
    fn decode_peer_frame(&mut self, peer_addr: &str, raw: &[u8]) -> Result<IncomingFrame, SysinspectError> {
        let cfg = self.cfg.clone();
        let peer_transport = &mut self.peer_transport;
        let mkr = &mut self.mkr;
        peer_transport.decode_frame(peer_addr, raw, &cfg, mkr)
    }

    /// XXX: That needs to be out to the telemetry::otel::OtelLogger instead!
    async fn on_log_previous_query(&mut self, msg: &MasterMessage) {
        let scheme = msg.target().scheme();
        if !scheme.contains("/") {
            log::debug!("No model scheme found");
            return;
        }

        // Skip command scheme
        if scheme.starts_with(SCHEME_COMMAND) {
            return;
        }
        let scheme = scheme.split('/').next().unwrap_or_default();
        if scheme.is_empty() {
            log::error!("No model scheme found");
            return;
        }

        let mut reducer =
            match FunctionReducer::new(self.cfg().fileserver_root().join(format!("{CFG_MODELS_ROOT}/{scheme}/model.cfg")), scheme.to_string())
                .load_model(&MODEL_CACHE)
                .await
            {
                Ok(reducer) => reducer,
                Err(err) => {
                    log::error!("Unable to load model: {err}");
                    return;
                }
            };

        if let Ok(s) = self.evtipc.get_last_session().await {
            for m in self.evtipc.get_minions(s.sid()).await.unwrap_or_default() {
                let mrec = match self.mreg.lock().await.get(m.id()) {
                    Ok(Some(mrec)) => mrec,
                    Ok(None) => {
                        log::error!("Unable to get minion record");
                        continue;
                    }
                    Err(err) => {
                        log::error!("Unable to get minion record: {err}");
                        continue;
                    }
                };

                if let Ok(events) = self.evtipc.get_events(s.sid(), m.id()).await {
                    for e in events {
                        reducer.feed(mrec.clone(), e);
                    }
                }
            }
        }
        reducer.map();
        reducer.reduce();

        if self.cfg.telemetry_enabled() {
            // Emit reduced data
            for (mid, res) in reducer.get_reduced_data() {
                if let Ok(Some(mrec)) = self.mreg.lock().await.get(mid) {
                    let fqdn = mrec.get_traits().get("system.hostname.fqdn").unwrap_or(&serde_json::Value::String("".to_string())).to_string();
                    libtelemetry::otel_log_json(res, vec![("hostname".into(), fqdn.into())]);
                } else {
                    log::error!("Minion {mid} has a data, but no minion record found");
                }
            }
        }
    }

    /// Construct a Command message to the minion
    pub(crate) async fn msg_query(&mut self, payload: &str) -> Option<MasterMessage> {
        let query = payload.split(";").map(|s| s.to_string()).collect::<Vec<String>>();
        if let [querypath, query, traits, mid, context] = query.as_slice() {
            return self.msg_query_data(querypath, query, traits, mid, context).await;
        }

        None
    }

    async fn msg_query_data(&mut self, querypath: &str, query: &str, traits: &str, mid: &str, context: &str) -> Option<MasterMessage> {
        let is_virtual = query.to_lowercase().starts_with("v:");
        let query = query.to_lowercase().replace("v:", "");

        log::debug!("Context: {context}");

        let hostnames: Vec<String> = query.split(',').map(|h| h.to_string()).collect();
        let mut tgt = MinionTarget::new(mid, "");
        tgt.set_scheme(querypath);
        tgt.set_context_query(context);

        log::debug!(
            "Querying minions for: {}, traits: {}, is virtual: {}",
            query.bright_yellow(),
            traits.bright_yellow(),
            if is_virtual { "yes".bright_green() } else { "no".bright_red() }
        );

        let mut targeted = false;
        if is_virtual && let Some(decided) = self.vmcluster.decide(&query, traits).await {
            for hostname in decided.iter() {
                log::debug!("Virtual minion requested. Decided to run on a physical: {}", hostname.bright_yellow());
                tgt.add_hostname(hostname);
                if !targeted {
                    targeted = true;
                }
            }
        } else if !is_virtual {
            for hostname in hostnames.iter() {
                tgt.add_hostname(hostname);
                if !targeted {
                    targeted = true;
                }
            }
            tgt.set_traits_query(traits);
        }

        if !targeted {
            log::warn!(
                "No suitable {}minion found for the query: {}, traits query: {}, context: {}",
                if is_virtual { "virtual " } else { "" },
                if query.is_empty() { "<N/A>".red() } else { query.bright_yellow() },
                if traits.is_empty() { "<N/A>".red() } else { traits.bright_yellow() },
                if context.is_empty() { "<N/A>".red() } else { context.bright_yellow() }
            );
            return None;
        }
        log::debug!("Target: {:#?}", tgt);

        let mut out: IndexMap<String, String> = IndexMap::default();
        for em in self.cfg.fileserver_models() {
            for (n, cs) in scan_files_sha256(self.cfg.fileserver_models_root(false).join(em), Some(MODEL_FILE_EXT)) {
                out.insert(format!("/{}/{em}/{n}", self.cfg.fileserver_models_root(false).file_name().unwrap().to_str().unwrap()), cs);
            }
        }

        let mut payload = String::new();
        if tgt.scheme().eq(SCHEME_COMMAND) {
            payload = query.to_owned();
        }

        let mut msg = MasterMessage::new(
            RequestType::Command,
            json!(
                ModStatePayload::new(payload)
                    .set_uri(querypath.to_string())
                    .add_files(out)
                    .set_models_root(self.cfg.fileserver_models_root(true).to_str().unwrap_or_default())
            ),
        );
        msg.set_target(tgt);
        msg.set_retcode(ProtoErrorCode::Success);

        log::debug!("Constructed message: {:#?}", msg);

        Some(msg)
    }

    fn msg_sensors_files(&mut self) -> MasterMessage {
        let sroot = self.cfg.fileserver_sensors_root();
        let ok = crate::util::log_sensors_export(&self.cfg, false);

        let mut out: IndexMap<String, String> = IndexMap::default();
        for es in ok {
            for (n, cs) in scan_files_sha256(sroot.join(&es), None) {
                out.insert(format!("/{}/{es}/{n}", sroot.file_name().unwrap().to_str().unwrap()), cs);
            }
        }

        MasterMessage::new(
            RequestType::SensorsSyncResponse,
            json!(
                ModStatePayload::new(String::from(""))
                    .add_files(out)
                    .set_sensors_root(sroot.file_name().unwrap_or_default().to_str().unwrap_or_default())
            ),
        )
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
        let mut m = MasterMessage::new(RequestType::AgentUnknown, json!(self.mkr().get_master_key_pem().clone().unwrap().to_string()));
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

    fn msg_bye_ack(&mut self, mid: String, sid: String) -> MasterMessage {
        let mut m = MasterMessage::new(RequestType::ByeAck, json!(sid));
        m.set_target(MinionTarget::new(&mid, &sid));
        m.set_retcode(ProtoErrorCode::Success);

        m
    }

    /// Get session keeper
    pub fn get_session(&self) -> Arc<Mutex<session::SessionKeeper>> {
        Arc::clone(&self.session)
    }

    /// Get minion registry
    pub fn get_minion_registry(&self) -> Arc<Mutex<MinionRegistry>> {
        Arc::clone(&self.mreg)
    }

    /// Get task registry
    pub fn get_task_registry(&self) -> Arc<Mutex<TaskRegistry>> {
        Arc::clone(&self.taskreg)
    }

    /// Clear session and transport state for one disconnected peer address.
    async fn on_peer_disconnect(&mut self, minion_addr: &str) {
        if let Some(mid) = self.conn_to_mid.remove(minion_addr) {
            log::info!("Minion connection {} dropped; clearing session for {}", minion_addr, mid);
            self.get_session().lock().await.remove(&mid);
        } else {
            log::debug!("Disconnect from {}, but no minion id mapped yet", minion_addr);
        }
        self.peer_transport.remove_peer(minion_addr);
    }

    /// Process a plaintext registration request and emit the one-shot registration response.
    async fn on_registration_request(&mut self, minion_addr: &str, minion_id: &str, payload: &str, bcast: &broadcast::Sender<MasterMessage>) {
        log::info!("Minion \"{minion_addr}\" requested registration");
        self.peer_transport.allow_plaintext(minion_addr);
        let resp_msg = if !self.mkr().is_registered(minion_id) {
            if let Err(err) = self.mkr().add_mn_key(minion_id, minion_addr, payload) {
                log::error!("Unable to add minion RSA key: {err}");
            }
            self.to_drop.insert(minion_addr.to_owned());
            log::info!("Registered a minion at {minion_addr} ({minion_id})");
            "Minion registration has been accepted"
        } else {
            log::warn!("Minion {minion_addr} ({minion_id}) is already registered");
            "Minion already registered"
        };
        _ = bcast.send(self.msg_registered(minion_id.to_string(), resp_msg));
    }

    /// Process an `ehlo` request and either establish the runtime session or reject the peer.
    async fn on_ehlo_request(&mut self, minion_addr: &str, minion_id: &str, payload: &str, bcast: &broadcast::Sender<MasterMessage>) {
        log::info!("EHLO from {}", minion_id);
        if !self.mkr().is_registered(minion_id) {
            log::info!("Minion at {minion_addr} ({minion_id}) is not registered");
            self.to_drop.insert(minion_addr.to_owned());
            _ = bcast.send(self.msg_not_registered(minion_id.to_string()));
            return;
        }
        if self.get_session().lock().await.exists(minion_id) {
            log::info!("Minion at {minion_addr} ({minion_id}) is already connected");
            self.to_drop.insert(minion_addr.to_owned());
            _ = bcast.send(self.msg_already_connected(minion_id.to_string(), payload.to_string()));
            return;
        }

        log::info!("{minion_id} connected successfully");
        self.conn_to_mid.insert(minion_addr.to_string(), minion_id.to_string());
        self.get_session().lock().await.ping(minion_id, Some(payload));
        _ = bcast.send(self.msg_request_traits(minion_id.to_string(), payload.to_string()));
        log::info!("Syncing traits with minion at {minion_id}");

        match self.pending_rotation_message_for(minion_id).await {
            Ok(Some(msg)) => {
                log::info!("Dispatching deferred rotation to {} after reconnect", minion_id.bright_yellow());
                _ = bcast.send(msg);
            }
            Ok(None) => {}
            Err(err) => {
                log::error!("Unable to dispatch deferred rotation for {}: {err}", minion_id);
            }
        }
    }

    /// Process a `pong` heartbeat update from one minion.
    async fn on_pong_request(&mut self, minion_id: &str, payload: serde_json::Value) {
        log::debug!("Received pong from {payload:#?}");
        let pm = match PingData::from_value(payload) {
            Ok(pm) => pm,
            Err(err) => {
                log::error!("Unable to parse pong message: {err}");
                return;
            }
        };

        self.get_session().lock().await.ping(minion_id, Some(pm.sid()));
        self.vmcluster.update_stats(minion_id, pm.payload().load_average(), pm.payload().disk_write_bps(), pm.payload().cpu_usage());

        let taskreg = self.get_task_registry();
        let mut taskreg = taskreg.lock().await;
        taskreg.flush(minion_id, pm.payload().completed());
    }

    /// Process a `bye` request and acknowledge the disconnect.
    async fn on_bye_request(&mut self, minion_addr: &str, minion_id: &str, payload: &str, bcast: &broadcast::Sender<MasterMessage>) {
        log::info!("Minion {} disconnects", minion_id);
        self.conn_to_mid.remove(minion_addr);
        self.get_session().lock().await.remove(minion_id);
        self.peer_transport.remove_peer(minion_addr);
        _ = bcast.send(self.msg_bye_ack(minion_id.to_string(), payload.to_string()));
    }

    /// Dispatch one parsed minion request into the dedicated async handler for that request family.
    fn spawn_incoming_request(master: Arc<Mutex<Self>>, bcast: broadcast::Sender<MasterMessage>, req: MinionMessage, minion_addr: String) {
        match req.req_type() {
            RequestType::Add => {
                let c_master = Arc::clone(&master);
                let c_bcast = bcast.clone();
                let c_mid = req.id().to_string();
                let c_payload = req.payload().to_string();
                tokio::spawn(async move {
                    c_master.lock().await.on_registration_request(&minion_addr, &c_mid, &c_payload, &c_bcast).await;
                });
            }
            RequestType::Response => {
                log::info!("Response");
            }
            RequestType::Ehlo => {
                let c_master = Arc::clone(&master);
                let c_bcast = bcast.clone();
                let c_id = req.id().to_string();
                let c_payload = req.payload().to_string();
                tokio::spawn(async move {
                    c_master.lock().await.on_ehlo_request(&minion_addr, &c_id, &c_payload, &c_bcast).await;
                });
            }
            RequestType::Pong => {
                let c_master = Arc::clone(&master);
                let c_id = req.id().to_string();
                let c_payload = req.payload().clone();
                tokio::spawn(async move {
                    c_master.lock().await.on_pong_request(&c_id, c_payload).await;
                });
            }
            RequestType::Traits => {
                log::debug!("Syncing traits from {}", req.id());
                let c_master = Arc::clone(&master);
                let c_id = req.id().to_string();
                let c_payload = req.payload().to_string();
                tokio::spawn(async move {
                    c_master.lock().await.on_traits(c_id, c_payload).await;
                });
            }
            RequestType::Bye => {
                let c_master = Arc::clone(&master);
                let c_bcast = bcast.clone();
                let c_id = req.id().to_string();
                let c_payload = req.payload().to_string();
                tokio::spawn(async move {
                    c_master.lock().await.on_bye_request(&minion_addr, &c_id, &c_payload, &c_bcast).await;
                });
            }
            RequestType::Event => {
                let c_master = Arc::clone(&master);
                tokio::spawn(async move {
                    log::debug!("Event for {}: {}", req.id(), req.payload());
                    let d = req.get_data();
                    let m = c_master.lock().await;
                    m.taskreg.lock().await.deregister(d.cid(), req.id());
                    let mrec = m.mreg.lock().await.get(req.id()).unwrap_or_default().unwrap_or_default();

                    let pl = match serde_json::from_str::<HashMap<String, serde_json::Value>>(req.payload().to_string().as_str()) {
                        Ok(pl) => pl,
                        Err(err) => {
                            log::error!("An event message with the bogus payload: {err}");
                            return;
                        }
                    };

                    if m.cfg().telemetry_enabled() {
                        OtelLogger::new(&pl).log(&mrec, DataExportType::Action);
                    }

                    let sid = match m
                        .evtipc
                        .open_session(
                            util::dataconv::as_str(pl.get(&ProtoKey::EntityId.to_string()).cloned()),
                            util::dataconv::as_str(pl.get(&ProtoKey::CycleId.to_string()).cloned()),
                            util::dataconv::as_str(pl.get(&ProtoKey::Timestamp.to_string()).cloned()),
                        )
                        .await
                    {
                        Ok(sid) => sid,
                        Err(err) => {
                            log::debug!("Unable to acquire session for this iteration: {err}");
                            return;
                        }
                    };

                    let mid = match m.evtipc.ensure_minion(&sid, req.id().to_string(), mrec.get_traits().to_owned()).await {
                        Ok(mid) => mid,
                        Err(err) => {
                            log::error!("Unable to record a minion {}: {err}", req.id());
                            return;
                        }
                    };

                    match m.evtipc.add_event(&sid, EventMinion::new(mid), pl).await {
                        Ok(_) => {
                            log::debug!("Event added for {} in {:#?}", req.id(), sid);
                        }
                        Err(err) => {
                            log::error!("Unable to add event: {err}");
                        }
                    };
                });
            }
            RequestType::ModelEvent => {
                let c_master = Arc::clone(&master);
                tokio::spawn(async move {
                    let master = c_master.lock().await;
                    let mrec = master.mreg.lock().await.get(req.id()).unwrap_or_default().unwrap_or_default();

                    let pl = match serde_json::from_str::<HashMap<String, serde_json::Value>>(req.payload().to_string().as_str()) {
                        Ok(pl) => pl,
                        Err(err) => {
                            log::error!("An event message with the bogus payload: {err}");
                            return;
                        }
                    };
                    let sid = match master.evtipc.get_session(&util::dataconv::as_str(pl.get(&ProtoKey::CycleId.to_string()).cloned())).await {
                        Ok(sid) => sid,
                        Err(err) => {
                            log::debug!("Unable to acquire session for this iteration: {err}");
                            return;
                        }
                    };

                    if master.cfg().telemetry_enabled() {
                        let mut otel = OtelLogger::new(&pl);
                        otel.set_map(true);
                        match master.evtipc.get_events(sid.sid(), req.id()).await {
                            Ok(events) => match master.mreg.lock().await.get(req.id()) {
                                Ok(Some(mrec)) => {
                                    otel.feed(events, mrec);
                                }
                                Ok(None) => {
                                    log::error!("Unable to get minion record for {}", req.id());
                                }
                                Err(err) => {
                                    log::error!("Error retrieving minion record for {}: {}", req.id(), err);
                                }
                            },
                            Err(err) => {
                                log::error!("Error retrieving events for minion {}: {}", req.id(), err);
                            }
                        }
                        otel.log(&mrec, DataExportType::Model);
                    }
                });
            }
            RequestType::SensorsSyncRequest => {
                let c_master = Arc::clone(&master);
                let c_bcast = bcast.clone();
                tokio::spawn(async move {
                    let mut guard = c_master.lock().await;
                    _ = c_bcast.send(guard.msg_sensors_files());
                });
            }
            _ => {
                log::error!("Minion sends unknown request type");
            }
        }
    }

    /// Process incoming minion messages
    #[allow(clippy::while_let_loop)]
    pub async fn do_incoming(master: Arc<Mutex<Self>>, mut rx: tokio::sync::mpsc::Receiver<(Vec<u8>, String)>) {
        log::trace!("Init incoming channel");
        let bcast = master.lock().await.broadcast();
        tokio::spawn(async move {
            loop {
                if let Some((raw, minion_addr)) = rx.recv().await {
                    // Minion disconnects here
                    if raw.is_empty() {
                        let c_master = Arc::clone(&master);
                        tokio::spawn(async move {
                            c_master.lock().await.on_peer_disconnect(&minion_addr).await;
                        });
                        continue;
                    }

                    let msg = String::from_utf8_lossy(&raw).to_string();
                    log::debug!("Minion response: {minion_addr}: {msg}");

                    if let Some(req) = master.lock().await.to_request(&msg) {
                        Self::spawn_incoming_request(Arc::clone(&master), bcast.clone(), req, minion_addr.clone());
                    } else {
                        log::error!("Unable to parse minion message");
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
            let mut mreg = self.mreg.lock().await;
            if let Err(err) = mreg.refresh(&mid, traits) {
                log::error!("Unable to sync traits: {err}");
            } else {
                let m = mreg.get(&mid).unwrap_or_default().unwrap_or_default();
                log::info!(
                    "Traits synced for minion {} ({})",
                    m.get_traits().get("system.hostname.fqdn").and_then(|v| v.as_str()).unwrap_or("unknown").bright_green(),
                    mid.green()
                );
            }
        }
    }

    pub async fn on_fifo_commands(&mut self, msg: &MasterMessage) {
        if msg.target().scheme().eq(&format!("cmd://{}", CLUSTER_REMOVE_MINION)) && !msg.target().id().is_empty() {
            log::info!("Removing minion {}", msg.target().id());
            if let Err(err) = self.mreg.lock().await.remove(msg.target().id()) {
                log::error!("Unable to remove minion {}: {err}", msg.target().id());
            }
            if let Err(err) = self.mkr().remove_mn_key(msg.target().id()) {
                log::error!("Unable to unregister minion: {err}");
            }
        } else if msg.target().scheme().eq(&format!("cmd://{}", CLUSTER_ONLINE_MINIONS)) {
            // XXX: This is just a logdumper for now, because there is no proper response channel yet.
            // Most likely we need to dump FIFO mechanism in a whole and replace with something else.
            log::info!("Listing online minions");
            let mreg = self.mreg.lock().await;
            let mut session = self.session.lock().await;
            match mreg.get_registered_ids() {
                Ok(ids) => {
                    let mut msg: Vec<String> = vec![];
                    for (idx, mid) in ids.iter().enumerate() {
                        let alive = session.alive(mid);
                        let traits = match mreg.get(mid) {
                            Ok(Some(mrec)) => mrec.get_traits().to_owned(),
                            _ => HashMap::new(),
                        };
                        let mut h = traits.get("system.hostname.fqdn").and_then(|v| v.as_str()).unwrap_or("unknown");
                        if h.is_empty() {
                            h = traits.get("system.hostname").and_then(|v| v.as_str()).unwrap_or("unknown");
                        }
                        let ip = traits.get("system.hostname.ip").and_then(|v| v.as_str()).unwrap_or("unknown");

                        msg.push(format!(
                            "{}. {} {} - {} ({})",
                            idx + 1,
                            if alive { " " } else { "!" },
                            if alive { mid.cyan() } else { mid.white() },
                            if alive { h.bright_green() } else { h.yellow() },
                            if alive { ip.bright_green() } else { ip.yellow() },
                        ));
                    }
                    log::info!("Status of all registered minions:\n{}", msg.join("\n"));
                }
                Err(err) => {
                    log::error!("Unable to get online minions: {err}");
                }
            }
        }
    }

    /// Build the formatted online-minion summary returned by the console `--online` command.
    async fn online_minions_summary(&mut self) -> Result<String, SysinspectError> {
        let mreg = self.mreg.lock().await;
        let mut session = self.session.lock().await;
        let ids = mreg.get_registered_ids()?;
        let mut rows: Vec<(String, String, String, String, String, String)> = vec![];

        for mid in &ids {
            let alive = session.alive(mid);
            let traits = match mreg.get(mid) {
                Ok(Some(mrec)) => mrec.get_traits().to_owned(),
                _ => HashMap::new(),
            };
            let mut h = traits.get("system.hostname.fqdn").and_then(|v| v.as_str()).unwrap_or("unknown");
            if h.is_empty() {
                h = traits.get("system.hostname").and_then(|v| v.as_str()).unwrap_or("unknown");
            }
            let ip = traits.get("system.hostname.ip").and_then(|v| v.as_str()).unwrap_or("unknown");
            let mid_short = if mid.chars().count() > 8 { format!("{}...{}", &mid[..4], &mid[mid.len() - 4..]) } else { mid.to_string() };
            rows.push((
                h.to_string(),
                if alive { h.bright_green().to_string() } else { h.red().to_string() },
                ip.to_string(),
                if alive { ip.bright_blue().to_string() } else { ip.blue().to_string() },
                mid_short.clone(),
                if alive { mid_short.bright_green().to_string() } else { mid_short.green().to_string() },
            ));
        }

        let host_width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(4).max("HOST".chars().count());
        let ip_width = rows.iter().map(|r| r.2.chars().count()).max().unwrap_or(2).max("IP".chars().count());
        let id_width = rows.iter().map(|r| r.4.chars().count()).max().unwrap_or(2).max("ID".chars().count());

        let mut out = vec![
            format!(
                "{}  {}  {}",
                pad_visible(&"HOST".bright_yellow().to_string(), host_width),
                pad_visible(&"IP".bright_yellow().to_string(), ip_width),
                pad_visible(&"ID".bright_yellow().to_string(), id_width),
            ),
            format!("{}  {}  {}", "─".repeat(host_width), "─".repeat(ip_width), "─".repeat(id_width)),
        ];

        for (_, host, _, ip, _, mid) in rows {
            out.push(format!("{}  {}  {}", pad_visible(&host, host_width), pad_visible(&ip, ip_width), pad_visible(&mid, id_width),));
        }

        Ok(out.join("\n"))
    }

    /// Resolve target minions for console profile operations from id, traits query, or hostname query.
    async fn selected_minions(&mut self, query: &str, traits: &str, mid: &str) -> Result<Vec<crate::registry::rec::MinionRecord>, SysinspectError> {
        let mut records = if !mid.is_empty() {
            self.mreg.lock().await.get(mid)?.into_iter().collect::<Vec<_>>()
        } else if !traits.trim().is_empty() {
            let traits = get_context(traits)
                .ok_or_else(|| SysinspectError::InvalidQuery("Traits selector must be in key:value format".to_string()))?
                .into_iter()
                .collect::<HashMap<_, _>>();
            self.mreg.lock().await.get_by_traits(traits)?
        } else {
            self.mreg.lock().await.get_by_query(if query.trim().is_empty() { "*" } else { query })?
        };
        records.sort_by(|a, b| a.id().cmp(b.id()));
        Ok(records)
    }

    /// Ensure a profile request carries a non-empty profile name when the operation requires one.
    fn require_profile_name(request: &ProfileConsoleRequest) -> Result<(), SysinspectError> {
        if !request.name().trim().is_empty() {
            return Ok(());
        }

        Err(SysinspectError::InvalidQuery("Profile name cannot be empty".to_string()))
    }

    /// Extract the current non-default profile list from one minion record.
    fn current_profiles(minion: &crate::registry::rec::MinionRecord) -> Vec<String> {
        let mut profiles = match minion.get_traits().get("minion.profile") {
            Some(serde_json::Value::String(name)) if !name.trim().is_empty() => vec![name.to_string()],
            Some(serde_json::Value::Array(names)) => names.iter().filter_map(|name| name.as_str().map(str::to_string)).collect::<Vec<_>>(),
            _ => vec![],
        };
        profiles.retain(|profile| profile != "default");
        profiles
    }

    /// Apply or remove profile tags for the selected minions and return the resulting outbound messages.
    async fn profile_tag_console_response(
        &mut self, request: &ProfileConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        let repo = SysInspectModPak::new(self.cfg.get_mod_repo_root())?;
        let known_profiles = repo.list_profiles(None)?;
        let missing = request.profiles().iter().filter(|name| !known_profiles.contains(name)).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Ok((
                ConsoleResponse {
                    ok: false,
                    message: format!("Unknown profile{}: {}", if missing.len() == 1 { "" } else { "s" }, missing.join(", ").bright_yellow()),
                },
                vec![],
            ));
        }

        let mut msgs = Vec::new();
        for minion in self.selected_minions(query, traits, mid).await? {
            let mut profiles = Self::current_profiles(&minion);
            if request.op() == "tag" {
                for profile in request.profiles() {
                    if !profiles.contains(profile) {
                        profiles.push(profile.to_string());
                    }
                }
            } else {
                profiles.retain(|profile| !request.profiles().contains(profile));
            }

            let context = if profiles.is_empty() {
                json!({"op": "unset", "traits": {"minion.profile": null}})
            } else {
                json!({"op": "set", "traits": {"minion.profile": profiles}})
            }
            .to_string();

            if let Some(msg) = self.msg_query_data(&format!("{SCHEME_COMMAND}{CLUSTER_TRAITS_UPDATE}"), "", "", minion.id(), &context).await {
                msgs.push(msg);
            }
        }

        Ok((
            ConsoleResponse {
                ok: true,
                message: format!(
                    "{} {} on {} minion{}",
                    if request.op() == "tag" { "Applied profiles" } else { "Removed profiles" },
                    request.profiles().join(", ").bright_yellow(),
                    msgs.len(),
                    if msgs.len() == 1 { "" } else { "s" }
                ),
            },
            msgs,
        ))
    }

    /// Execute one profile console request and return its console response plus any outbound master messages to broadcast.
    async fn do_profile_console(
        &mut self, request: &ProfileConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        let repo = SysInspectModPak::new(self.cfg.get_mod_repo_root())?;

        match request.op() {
            "new" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.new_profile(request.name())?;
                    ConsoleResponse { ok: true, message: format!("Created profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "delete" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.delete_profile(request.name())?;
                    ConsoleResponse { ok: true, message: format!("Deleted profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "list" => Ok((
                ConsoleResponse {
                    ok: true,
                    message: if request.name().is_empty() {
                        repo.list_profiles(None)?.join("\n")
                    } else {
                        repo.list_profile_matches(Some(request.name()), request.library())?.join("\n")
                    },
                },
                vec![],
            )),
            "show" => Ok((
                {
                    Self::require_profile_name(request)?;
                    ConsoleResponse { ok: true, message: repo.show_profile(request.name())? }
                },
                vec![],
            )),
            "add" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.add_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse { ok: true, message: format!("Updated profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "remove" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.remove_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse { ok: true, message: format!("Updated profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "tag" | "untag" => self.profile_tag_console_response(request, query, traits, mid).await,
            _ => Ok((ConsoleResponse { ok: false, message: format!("Unsupported profile operation {}", request.op().bright_yellow()) }, vec![])),
        }
    }

    fn master_rotator(&mut self, minion_id: &str) -> Result<RsaTransportRotator, SysinspectError> {
        let master_fp = self.mkr().get_master_key_fingerprint()?;
        let minion_fp = self.mkr().get_mn_key_fingerprint(minion_id)?;
        let store = TransportStore::for_master_minion(&self.cfg, minion_id)?;
        RsaTransportRotator::new(RotationActor::Master, store, minion_id, &master_fp, &minion_fp, SECURE_PROTOCOL_VERSION)
    }

    fn stage_rotation_context(&mut self, minion_id: &str, request: &RotationConsoleRequest) -> Result<String, SysinspectError> {
        let rotator = self.master_rotator(minion_id)?;
        let plan = rotator.plan(request.reason());
        let signed = rotator.sign_plan(&plan, &self.mkr().master_private_key()?)?;
        serde_json::to_string(&RotationCommandPayload {
            op: "rotate".to_string(),
            reason: request.reason().to_string(),
            grace_seconds: request.grace_seconds(),
            reconnect: request.reconnect(),
            reregister: request.reregister(),
            intent: signed,
        })
        .map_err(|err| SysinspectError::SerializationError(format!("Failed to encode rotate payload: {err}")))
    }

    fn persist_pending_rotation_context(&mut self, minion_id: &str, context: Option<String>) -> Result<(), SysinspectError> {
        let store = TransportStore::for_master_minion(&self.cfg, minion_id)?;
        let mut state = store.load()?.ok_or_else(|| SysinspectError::ProtoError(format!("No managed transport state exists for {minion_id}")))?;
        state.set_pending_rotation_context(context);
        store.save(&state)
    }

    async fn build_rotation_message(
        &mut self, minion_id: &str, request: &RotationConsoleRequest, reason_suffix: Option<&str>,
    ) -> Result<Option<MasterMessage>, SysinspectError> {
        if !self.mkr().is_registered(minion_id) {
            return Ok(None);
        }

        let mut requested = request.clone();
        if let Some(suffix) = reason_suffix {
            requested.reason = Some(format!("{}:{suffix}", request.reason()));
        }
        let context = self.stage_rotation_context(minion_id, &requested)?;
        self.persist_pending_rotation_context(minion_id, Some(context.clone()))?;
        let msg = self.msg_query_data(&format!("{SCHEME_COMMAND}{CLUSTER_ROTATE}"), "", "", minion_id, &context).await;
        if msg.is_none() {
            self.persist_pending_rotation_context(minion_id, None)?;
        }
        Ok(msg)
    }

    async fn rotate_console_response(
        &mut self, request: &RotationConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        if request.op() != "rotate" {
            return Ok((ConsoleResponse { ok: false, message: format!("Unsupported rotate operation {}", request.op().bright_yellow()) }, vec![]));
        }

        let mut online_msgs = Vec::new();
        let mut summary = RotationDispatchSummary::default();

        let targets = self.selected_minions(query, traits, mid).await?;
        for minion in targets {
            let minion_id = minion.id().to_string();
            let online = self.session.lock().await.alive(&minion_id);
            if online {
                if let Some(msg) = self.build_rotation_message(&minion_id, request, None).await? {
                    online_msgs.push(msg);
                    summary.note_online_dispatch();
                }
            } else {
                let context = self.stage_rotation_context(&minion_id, request)?;
                self.persist_pending_rotation_context(&minion_id, Some(context))?;
                summary.note_queued_dispatch();
            }
        }

        Ok((summary.response(), online_msgs))
    }

    async fn transport_status_summary(&mut self, query: &str, traits: &str, mid: &str) -> Result<String, SysinspectError> {
        let targets = self.selected_minions(query, traits, mid).await?;
        let mut lines = TransportStatusRow::header().into_iter().collect::<Vec<_>>();

        let now = Utc::now();
        for minion in targets {
            let minion_id = minion.id().to_string();
            let state = TransportStore::for_master_minion(&self.cfg, &minion_id)?.load()?;
            if let Some(state) = state {
                lines.push(TransportStatusRow::from_state(&minion_id, &state, now).render());
            } else {
                lines.push(TransportStatusRow::missing(&minion_id).render());
            }
        }

        Ok(lines.join("\n"))
    }

    async fn pending_rotation_message_for(&mut self, minion_id: &str) -> Result<Option<MasterMessage>, SysinspectError> {
        let store = TransportStore::for_master_minion(&self.cfg, minion_id)?;
        let state = match store.load()? {
            Some(state) => state,
            None => return Ok(None),
        };
        if !matches!(state.rotation, libsysinspect::transport::TransportRotationStatus::Pending)
            || state.pending_rotation_context.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
        {
            return Ok(None);
        }

        self.msg_query_data(
            &format!("{SCHEME_COMMAND}{CLUSTER_ROTATE}"),
            "",
            "",
            minion_id,
            state.pending_rotation_context.as_deref().unwrap_or_default(),
        )
        .await
        .ok_or_else(|| SysinspectError::ProtoError(format!("Unable to construct deferred rotation message for {minion_id}")))
        .map(Some)
    }

    /// Start the encrypted TCP console listener used by `sysinspect` to talk to the master.
    pub async fn do_console(master: Arc<Mutex<Self>>) {
        log::trace!("Init local console channel");
        tokio::spawn({
            let master = Arc::clone(&master);
            async move {
                let (cfg, bcast) = {
                    let guard = master.lock().await;
                    (guard.cfg(), guard.broadcast().clone())
                };
                let master_prk = match load_master_private_key(&cfg) {
                    Ok(prk) => prk,
                    Err(err) => {
                        log::error!("Failed to load console private key: {err}");
                        return;
                    }
                };
                let listener = match TcpListener::bind(cfg.console_listen_addr()).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        log::error!("Failed to bind console listener: {err}");
                        return;
                    }
                };
                loop {
                    match listener.accept().await {
                        Ok((stream, peer)) => {
                            let master = Arc::clone(&master);
                            let cfg = cfg.clone();
                            let bcast = bcast.clone();
                            let master_prk = master_prk.clone();
                            tokio::spawn(async move {
                                let (read_half, mut write_half) = stream.into_split();
                                let reader = TokioBufReader::new(read_half);
                                let mut frame = Vec::new();
                                let mut reader = reader.take((MAX_CONSOLE_FRAME_SIZE + 1) as u64);
                                let reply = match time::timeout(CONSOLE_READ_TIMEOUT, reader.read_until(b'\n', &mut frame)).await {
                                    Err(_) => serde_json::to_string(&ConsoleResponse {
                                        ok: false,
                                        message: format!("Console request timed out after {} seconds", CONSOLE_READ_TIMEOUT.as_secs()),
                                    })
                                    .ok(),
                                    Ok(Ok(0)) => {
                                        serde_json::to_string(&ConsoleResponse { ok: false, message: "Empty console request".to_string() }).ok()
                                    }
                                    Ok(Ok(_)) if frame.len() > MAX_CONSOLE_FRAME_SIZE || !frame.ends_with(b"\n") => {
                                        serde_json::to_string(&ConsoleResponse {
                                            ok: false,
                                            message: format!("Console request exceeds {} bytes", MAX_CONSOLE_FRAME_SIZE),
                                        })
                                        .ok()
                                    }
                                    Ok(Ok(_)) => match String::from_utf8(frame).map(|line| line.trim().to_string()) {
                                        Ok(line) => match serde_json::from_str::<ConsoleEnvelope>(&line) {
                                            Ok(envelope) => {
                                                if !authorised_console_client(&cfg, &envelope.bootstrap.client_pubkey).unwrap_or(false) {
                                                    serde_json::to_string(&ConsoleResponse {
                                                        ok: false,
                                                        message: "Console client key is not authorised".to_string(),
                                                    })
                                                    .ok()
                                                } else {
                                                    match envelope.bootstrap.session_key(&master_prk) {
                                                        Ok((key, _client_pkey)) => {
                                                            let response = match envelope.sealed.open::<ConsoleQuery>(&key) {
                                                                Ok(query) => {
                                                                    if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}")) {
                                                                        match master.lock().await.online_minions_summary().await {
                                                                            Ok(summary) => ConsoleResponse { ok: true, message: summary },
                                                                            Err(err) => ConsoleResponse {
                                                                                ok: false,
                                                                                message: format!("Unable to get online minions: {err}"),
                                                                            },
                                                                        }
                                                                    } else if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_TRANSPORT_STATUS}")) {
                                                                        match master
                                                                            .lock()
                                                                            .await
                                                                            .transport_status_summary(&query.query, &query.traits, &query.mid)
                                                                            .await
                                                                        {
                                                                            Ok(summary) => ConsoleResponse { ok: true, message: summary },
                                                                            Err(err) => ConsoleResponse {
                                                                                ok: false,
                                                                                message: format!("Unable to get transport status: {err}"),
                                                                            },
                                                                        }
                                                                    } else if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_ROTATE}")) {
                                                                        let (response, msgs) =
                                                                            match RotationConsoleRequest::from_context(&query.context) {
                                                                                Ok(request) => {
                                                                                    let mut guard = master.lock().await;
                                                                                    match guard
                                                                                        .rotate_console_response(
                                                                                            &request,
                                                                                            &query.query,
                                                                                            &query.traits,
                                                                                            &query.mid,
                                                                                        )
                                                                                        .await
                                                                                    {
                                                                                        Ok(data) => data,
                                                                                        Err(err) => (
                                                                                            ConsoleResponse { ok: false, message: err.to_string() },
                                                                                            vec![],
                                                                                        ),
                                                                                    }
                                                                                }
                                                                                Err(err) => (
                                                                                    ConsoleResponse {
                                                                                        ok: false,
                                                                                        message: format!("Failed to parse rotate request: {err}"),
                                                                                    },
                                                                                    vec![],
                                                                                ),
                                                                            };
                                                                        for msg in msgs {
                                                                            SysMaster::bcast_master_msg(
                                                                                &bcast,
                                                                                cfg.telemetry_enabled(),
                                                                                Arc::clone(&master),
                                                                                Some(msg.clone()),
                                                                            )
                                                                            .await;
                                                                            let guard = master.lock().await;
                                                                            let ids = guard
                                                                                .mreg
                                                                                .lock()
                                                                                .await
                                                                                .get_targeted_minions(msg.target(), false)
                                                                                .await;
                                                                            guard.taskreg.lock().await.register(msg.cycle(), ids);
                                                                        }
                                                                        response
                                                                    } else if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}")) {
                                                                        let (response, msgs) =
                                                                            match ProfileConsoleRequest::from_context(&query.context) {
                                                                                Ok(request) => {
                                                                                    let mut guard = master.lock().await;
                                                                                    match guard
                                                                                        .do_profile_console(
                                                                                            &request,
                                                                                            &query.query,
                                                                                            &query.traits,
                                                                                            &query.mid,
                                                                                        )
                                                                                        .await
                                                                                    {
                                                                                        Ok(data) => data,
                                                                                        Err(err) => (
                                                                                            ConsoleResponse { ok: false, message: err.to_string() },
                                                                                            vec![],
                                                                                        ),
                                                                                    }
                                                                                }
                                                                                Err(err) => (
                                                                                    ConsoleResponse {
                                                                                        ok: false,
                                                                                        message: format!("Failed to parse profile request: {err}"),
                                                                                    },
                                                                                    vec![],
                                                                                ),
                                                                            };
                                                                        for msg in msgs {
                                                                            SysMaster::bcast_master_msg(
                                                                                &bcast,
                                                                                cfg.telemetry_enabled(),
                                                                                Arc::clone(&master),
                                                                                Some(msg.clone()),
                                                                            )
                                                                            .await;
                                                                            let guard = master.lock().await;
                                                                            let ids = guard
                                                                                .mreg
                                                                                .lock()
                                                                                .await
                                                                                .get_targeted_minions(msg.target(), false)
                                                                                .await;
                                                                            guard.taskreg.lock().await.register(msg.cycle(), ids);
                                                                        }
                                                                        response
                                                                    } else {
                                                                        let msg = {
                                                                            let mut guard = master.lock().await;
                                                                            guard
                                                                                .msg_query_data(
                                                                                    &query.model,
                                                                                    &query.query,
                                                                                    &query.traits,
                                                                                    &query.mid,
                                                                                    &query.context,
                                                                                )
                                                                                .await
                                                                        };
                                                                        if let Some(msg) = msg {
                                                                            SysMaster::bcast_master_msg(
                                                                                &bcast,
                                                                                cfg.telemetry_enabled(),
                                                                                Arc::clone(&master),
                                                                                Some(msg.clone()),
                                                                            )
                                                                            .await;
                                                                            let guard = master.lock().await;
                                                                            let ids = guard
                                                                                .mreg
                                                                                .lock()
                                                                                .await
                                                                                .get_targeted_minions(msg.target(), false)
                                                                                .await;
                                                                            guard.taskreg.lock().await.register(msg.cycle(), ids);
                                                                            ConsoleResponse {
                                                                                ok: true,
                                                                                message: format!("Accepted console command from {peer}"),
                                                                            }
                                                                        } else {
                                                                            ConsoleResponse {
                                                                                ok: false,
                                                                                message: "No message constructed for the console query".to_string(),
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Err(err) => ConsoleResponse {
                                                                    ok: false,
                                                                    message: format!("Failed to open console query: {err}"),
                                                                },
                                                            };
                                                            match ConsoleSealed::seal(&response, &key).and_then(|sealed| {
                                                                serde_json::to_string(&sealed)
                                                                    .map_err(|e| SysinspectError::SerializationError(e.to_string()))
                                                            }) {
                                                                Ok(reply) => Some(reply),
                                                                Err(err) => {
                                                                    log::error!("Failed to seal console response: {err}");
                                                                    serde_json::to_string(&ConsoleResponse {
                                                                        ok: false,
                                                                        message: format!("Failed to seal console response: {err}"),
                                                                    })
                                                                    .ok()
                                                                }
                                                            }
                                                        }
                                                        Err(err) => serde_json::to_string(&ConsoleResponse {
                                                            ok: false,
                                                            message: format!("Console bootstrap failed: {err}"),
                                                        })
                                                        .ok(),
                                                    }
                                                }
                                            }
                                            Err(err) => serde_json::to_string(&ConsoleResponse {
                                                ok: false,
                                                message: format!("Failed to parse console request: {err}"),
                                            })
                                            .ok(),
                                        },
                                        Err(err) => serde_json::to_string(&ConsoleResponse {
                                            ok: false,
                                            message: format!("Console request is not valid UTF-8: {err}"),
                                        })
                                        .ok(),
                                    },
                                    Ok(Err(err)) => serde_json::to_string(&ConsoleResponse {
                                        ok: false,
                                        message: format!("Failed to read console request: {err}"),
                                    })
                                    .ok(),
                                };

                                if let Some(reply) = reply
                                    && let Err(err) = write_half.write_all(format!("{reply}\n").as_bytes()).await
                                {
                                    log::error!("Failed to write console response: {err}");
                                }
                            });
                        }
                        Err(err) => {
                            log::error!("Console listener accept error: {err}");
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });
    }

    /// Broadcast a message to all minions
    /// Broadcast a logical master message so each connected peer can encode it with its own transport state.
    pub async fn bcast_master_msg(
        bcast: &broadcast::Sender<MasterMessage>, use_telemetry: bool, master: Arc<Mutex<SysMaster>>, msg: Option<MasterMessage>,
    ) {
        if msg.is_none() {
            log::error!("No message to broadcast");
            return;
        }
        let msg = msg.unwrap();

        {
            let c_master = Arc::clone(&master);
            let c_msg = msg.clone();
            tokio::spawn(async move {
                let mut guard = c_master.lock().await;
                guard.on_fifo_commands(&c_msg).await;
            });
        }

        if use_telemetry {
            let c_master = Arc::clone(&master);
            let c_msg = msg.clone();
            tokio::spawn(async move {
                let mut guard = c_master.lock().await;
                guard.on_log_previous_query(&c_msg).await;
            });
            log::debug!("Telemetry enabled, fired a task");
        }

        log::debug!("Message broadcasted: {}", msg.target().scheme());
        let _ = bcast.send(msg);
    }

    pub async fn do_heartbeat(master: Arc<Mutex<Self>>) {
        log::trace!("Starting heartbeat");
        let bcast = master.lock().await.broadcast();
        tokio::spawn(async move {
            loop {
                _ = time::sleep(Duration::from_secs(5)).await;
                let mut p = MasterMessage::new(RequestType::Ping, json!(ProtoValue::PingTypeGeneral));
                let mut t = MinionTarget::default();
                t.add_hostname("*");
                p.set_target(t);
                let _ = bcast.send(p);
            }
        });
    }

    /// Encode one outbound frame for a connected peer, skipping broadcasts until the peer is allowed to receive them.
    async fn encode_outgoing_frame(&mut self, peer_addr: &str, frame: OutgoingFrame) -> Result<Option<Vec<u8>>, SysinspectError> {
        match frame {
            OutgoingFrame::Direct(msg) => Ok(Some(msg)),
            OutgoingFrame::Broadcast(msg) => {
                if !self.peer_transport.can_receive_broadcast(peer_addr) {
                    return Ok(None);
                }
                self.peer_transport.encode_message(peer_addr, &msg).map(Some)
            }
        }
    }

    /// Write direct replies and broadcast frames to one connected peer until the socket closes.
    async fn write_peer_frames(
        master: Arc<Mutex<Self>>, mut writer: OwnedWriteHalf, peer_addr: String, mut bcast_sub: broadcast::Receiver<MasterMessage>,
        mut direct_rx: mpsc::Receiver<Vec<u8>>, cancel_writer: tokio::sync::watch::Sender<bool>,
    ) {
        log::info!("Minion {peer_addr} connected. Ready to send messages.");

        loop {
            let frame = match tokio::select! {
                biased;
                Some(msg) = direct_rx.recv() => Some(OutgoingFrame::Direct(msg)),
                Ok(msg) = bcast_sub.recv() => Some(OutgoingFrame::Broadcast(Box::new(msg))),
                else => return,
            } {
                Some(frame) => frame,
                None => return,
            };

            let encoded = {
                let mut guard = master.lock().await;
                match guard.encode_outgoing_frame(&peer_addr, frame).await {
                    Ok(Some(msg)) => msg,
                    Ok(None) => continue,
                    Err(err) => {
                        log::error!("Failed to encode outbound message for {peer_addr}: {err}");
                        let _ = cancel_writer.send(true);
                        return;
                    }
                }
            };

            log::trace!("Sending message to minion at {} length of {}", peer_addr, encoded.len());
            let mut guard = master.lock().await;
            if writer.write_all(&(encoded.len() as u32).to_be_bytes()).await.is_err()
                || writer.write_all(&encoded).await.is_err()
                || writer.flush().await.is_err()
            {
                if let Err(err) = cancel_writer.send(true) {
                    log::debug!("Error sending cancel notification: {err}");
                }
                break;
            }

            if guard.to_drop.contains(&peer_addr) {
                guard.to_drop.remove(&peer_addr);
                log::info!("Dropping minion: {}", &peer_addr);
                if let Err(err) = writer.shutdown().await {
                    log::error!("Error shutting down outgoing: {err}");
                }
                if let Err(err) = cancel_writer.send(true) {
                    log::debug!("Error sending cancel notification: {err}");
                }
                return;
            }
        }
    }

    /// Read framed peer traffic, decode it through the peer transport object, and forward logical messages inward.
    async fn read_peer_frames(
        master: Arc<Mutex<Self>>, reader: OwnedReadHalf, peer_addr: String, client_tx: mpsc::Sender<(Vec<u8>, String)>,
        direct_tx: mpsc::Sender<Vec<u8>>, cancel_tx: tokio::sync::watch::Sender<bool>, cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let mut reader = TokioBufReader::new(reader);
        loop {
            if *cancel_rx.borrow() {
                log::info!("Process terminated");
                return;
            }

            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).await.is_err() {
                let _ = client_tx.send((Vec::new(), peer_addr.clone())).await;
                return;
            }

            let msg_len = u32::from_be_bytes(len_buf) as usize;
            let mut msg = vec![0u8; msg_len];
            if reader.read_exact(&mut msg).await.is_err() {
                let _ = client_tx.send((Vec::new(), peer_addr.clone())).await;
                return;
            }

            let decoded = {
                let mut guard = master.lock().await;
                guard.decode_peer_frame(&peer_addr, &msg)
            };
            match decoded {
                Ok(IncomingFrame::Forward(msg)) => {
                    if client_tx.send((msg, peer_addr.clone())).await.is_err() {
                        break;
                    }
                }
                Ok(IncomingFrame::Reply(msg)) => {
                    let _ = direct_tx.send(msg).await;
                }
                Err(err) => {
                    log::error!("Failed to decode frame from {peer_addr}: {err}");
                    let _ = cancel_tx.send(true);
                    let _ = client_tx.send((Vec::new(), peer_addr.clone())).await;
                    return;
                }
            }
        }
    }

    /// Spawn the paired reader and writer tasks for one accepted minion socket.
    async fn handle_peer_connection(
        master: Arc<Mutex<Self>>, tx: mpsc::Sender<(Vec<u8>, String)>, bcast: &broadcast::Sender<MasterMessage>, socket: tokio::net::TcpStream,
    ) {
        let bcast_sub = bcast.subscribe();
        let client_tx = tx.clone();
        let peer_addr = socket.peer_addr().unwrap().to_string();
        let writer_peer_addr = peer_addr.clone();
        let (reader, writer) = socket.into_split();
        let c_master_writer = Arc::clone(&master);
        let c_master_reader = Arc::clone(&master);
        let (direct_tx, direct_rx) = mpsc::channel::<Vec<u8>>(8);
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        let cancel_writer = cancel_tx.clone();

        tokio::spawn(async move {
            Self::write_peer_frames(c_master_writer, writer, writer_peer_addr, bcast_sub, direct_rx, cancel_writer).await;
        });

        tokio::spawn(async move {
            Self::read_peer_frames(c_master_reader, reader, peer_addr, client_tx, direct_tx, cancel_tx, cancel_rx).await;
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
                        Self::handle_peer_connection(Arc::clone(&master), tx.clone(), &bcast, socket).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Start scheduler
    async fn do_scheduler_service(master: Arc<Mutex<Self>>) -> Option<tokio::task::JoinHandle<()>> {
        let scheduler = master.lock().await.cfg().scheduler();
        if scheduler.is_empty() {
            log::info!("No recurring tasks defined");
            return None;
        }

        log::info!("Adding {} recurring tasks", scheduler.len());
        Some(tokio::spawn(async move {
            let svc = libscheduler::SchedulerService::new();
            for tdef in scheduler {
                let tname = tdef.name().to_string();
                let mtask = Arc::clone(&master);
                match libscheduler::pulse::EventTask::new(tdef.clone(), move || {
                    let master = Arc::clone(&mtask);
                    let tdef = tdef.clone();
                    async move {
                        let (bcast, msg, cfg) = {
                            let mut master = master.lock().await;
                            (master.broadcast().clone(), master.msg_query(tdef.query().as_str()).await, master.cfg().clone())
                        };
                        SysMaster::bcast_master_msg(&bcast, cfg.telemetry_enabled(), Arc::clone(&master), msg).await;
                    }
                }) {
                    Ok(etask) => {
                        log::info!("Task {tname} added");
                        svc.add_event(etask).await.unwrap();
                    }
                    Err(err) => {
                        log::error!("Unable to add task {tname}: {err}");
                        continue;
                    }
                };
            }
            svc.start().await.unwrap();
        }))
    }

    /// Start IPC server
    async fn do_ipc_service(master: Arc<Mutex<Self>>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let evtipc = Arc::clone(&master.lock().await.evtipc.clone());
            if let Err(e) = evtipc.run().await {
                log::error!("IPC server error: {e:?}");
            }
        })
    }
}

pub(crate) async fn master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let master = Arc::new(Mutex::new(SysMaster::new(cfg.clone())?));
    {
        let weak = Arc::downgrade(&master);
        master.lock().await.ptr = Some(weak);
    }
    {
        let mut m = master.lock().await;
        m.init().await?;
        log::info!("SysMaster initialized");
    }

    let (client_tx, client_rx) = mpsc::channel::<(Vec<u8>, String)>(100);

    // Start internal fileserver for minions
    fls::start(cfg.clone()).await?;
    log::info!("Fileserver started on directory {}", cfg.fileserver_root().to_str().unwrap_or_default());

    // Start web API (if configured/enabled)
    libwebapi::start_webapi(cfg.clone(), master.clone())?;

    // Start services
    let ipc = SysMaster::do_ipc_service(Arc::clone(&master)).await;
    let scheduler = SysMaster::do_scheduler_service(Arc::clone(&master)).await;
    libtelemetry::init_otel_collector(cfg).await?;

    SysMaster::do_console(Arc::clone(&master)).await;
    log::info!("Local console channel initialized");

    // Handle incoming messages from minions
    SysMaster::do_incoming(Arc::clone(&master), client_rx).await;
    log::info!("Incoming channel initialized");

    // Accept connections and spawn tasks for each client
    SysMaster::do_outgoing(Arc::clone(&master), client_tx).await?;
    log::info!("Outgoing channel initialized");

    SysMaster::do_heartbeat(Arc::clone(&master)).await;
    log::info!("Heartbeat service started");

    // Listen for shutdown signal and cancel tasks
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Received shutdown signal.");

    ipc.abort();

    if let Some(scheduler) = scheduler {
        scheduler.abort();
    }

    std::process::exit(0);
}
