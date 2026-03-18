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
};
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
    console::{ConsoleEnvelope, ConsoleQuery, ConsoleResponse, ConsoleSealed, authorised_console_client, ensure_console_keypair, load_master_private_key},
    context::{ProfileConsoleRequest, get_context},
    mdescr::{mspec::MODEL_FILE_EXT, mspecdef::ModelSpec, telemetry::DataExportType},
    util::{self, iofs::scan_files_sha256, pad_visible},
};
use libsysproto::{
    self, MasterMessage, MinionMessage, MinionTarget, ProtoConversion,
    errcodes::ProtoErrorCode,
    payload::{ModStatePayload, PingData},
    query::{
        SCHEME_COMMAND,
        commands::{CLUSTER_ONLINE_MINIONS, CLUSTER_PROFILE, CLUSTER_REMOVE_MINION, CLUSTER_TRAITS_UPDATE},
    },
    rqtypes::{ProtoKey, ProtoValue, RequestType},
};
use once_cell::sync::Lazy;
use serde_json::json;
use std::time::Duration as StdDuration;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Weak},
    vec,
};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, sleep};
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader},
    time,
};

// Session singleton
pub static SHARED_SESSION: Lazy<Arc<Mutex<SessionKeeper>>> = Lazy::new(|| Arc::new(Mutex::new(SessionKeeper::new(30))));
static MODEL_CACHE: Lazy<Arc<Mutex<HashMap<PathBuf, ModelSpec>>>> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
static MAX_CONSOLE_FRAME_SIZE: usize = 64 * 1024;
const CONSOLE_READ_TIMEOUT: StdDuration = StdDuration::from_secs(5);

#[derive(Debug)]
pub struct SysMaster {
    cfg: MasterConfig,
    broadcast: broadcast::Sender<Vec<u8>>,
    mkr: MinionsKeyRegistry,
    mreg: Arc<Mutex<MinionRegistry>>,
    taskreg: Arc<Mutex<TaskRegistry>>,
    evtipc: Arc<DbIPCService>,
    to_drop: HashSet<String>,
    session: Arc<Mutex<session::SessionKeeper>>,
    ptr: Option<Weak<Mutex<SysMaster>>>,
    vmcluster: VirtualMinionsCluster,
    conn_to_mid: HashMap<String, String>, // Map connection addresses to minion IDs
    datastore: Arc<Mutex<DataStorage>>,
}

impl SysMaster {
    pub fn new(cfg: MasterConfig) -> Result<SysMaster, SysinspectError> {
        let _ = crate::util::log_sensors_export(&cfg, true);

        let (tx, _) = broadcast::channel::<Vec<u8>>(100);
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
    pub fn broadcast(&self) -> broadcast::Sender<Vec<u8>> {
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
                            let mut guard = c_master.lock().await;

                            if let Some(mid) = guard.conn_to_mid.remove(&minion_addr) {
                                log::info!("Minion connection {} dropped; clearing session for {}", minion_addr, mid);
                                guard.get_session().lock().await.remove(&mid);
                            } else {
                                log::debug!("Disconnect from {}, but no minion id mapped yet", minion_addr);
                            }
                        });
                        continue;
                    }

                    let msg = String::from_utf8_lossy(&raw).to_string();
                    log::debug!("Minion response: {minion_addr}: {msg}");

                    if let Some(req) = master.lock().await.to_request(&msg) {
                        match req.req_type() {
                            RequestType::Add => {
                                let c_master = Arc::clone(&master);
                                let c_bcast = bcast.clone();
                                let c_mid = req.id().to_string();
                                tokio::spawn(async move {
                                    log::info!("Minion \"{minion_addr}\" requested registration");
                                    let mut guard = c_master.lock().await;
                                    let resp_msg: &str;
                                    if !guard.mkr().is_registered(&c_mid) {
                                        if let Err(err) = guard.mkr().add_mn_key(&c_mid, &minion_addr, &req.payload().to_string()) {
                                            log::error!("Unable to add minion RSA key: {err}");
                                        }
                                        guard.to_drop.insert(minion_addr.to_owned());
                                        resp_msg = "Minion registration has been accepted";
                                        log::info!("Registered a minion at {minion_addr} ({c_mid})");
                                    } else {
                                        resp_msg = "Minion already registered";
                                        log::warn!("Minion {minion_addr} ({c_mid}) is already registered");
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
                                    } else if guard.get_session().lock().await.exists(&c_id) {
                                        log::info!("Minion at {minion_addr} ({}) is already connected", req.id());
                                        guard.to_drop.insert(minion_addr);
                                        _ = c_bcast.send(guard.msg_already_connected(req.id().to_string(), c_payload).sendable().unwrap());
                                    } else {
                                        log::info!("{c_id} connected successfully");
                                        guard.conn_to_mid.insert(minion_addr.clone(), c_id.clone());
                                        guard.get_session().lock().await.ping(&c_id, Some(&c_payload));
                                        _ = c_bcast.send(guard.msg_request_traits(req.id().to_string(), c_payload).sendable().unwrap());
                                        log::info!("Syncing traits with minion at {c_id}");
                                    }
                                });
                            }

                            RequestType::Pong => {
                                let c_master = Arc::clone(&master);
                                let c_id = req.id().to_string();
                                tokio::spawn(async move {
                                    log::debug!("Received pong from {:#?}", req.payload());
                                    let mut guard = c_master.lock().await;
                                    let pm = match PingData::from_value(req.payload().clone()) {
                                        Ok(pm) => pm,
                                        Err(err) => {
                                            log::error!("Unable to parse pong message: {err}");
                                            return;
                                        }
                                    };

                                    guard.get_session().lock().await.ping(&c_id, Some(pm.sid()));
                                    guard.vmcluster.update_stats(
                                        &c_id,
                                        pm.payload().load_average(),
                                        pm.payload().disk_write_bps(),
                                        pm.payload().cpu_usage(),
                                    );

                                    // Update task tracker
                                    let taskreg = guard.get_task_registry();
                                    let mut taskreg = taskreg.lock().await;
                                    taskreg.flush(&c_id, pm.payload().completed());
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

                            RequestType::Bye => {
                                let c_master = Arc::clone(&master);
                                let c_bcast = bcast.clone();
                                log::info!("Minion {} disconnects", req.id());
                                tokio::spawn(async move {
                                    let mut guard = c_master.lock().await;
                                    guard.conn_to_mid.remove(&minion_addr);
                                    guard.get_session().lock().await.remove(req.id());
                                    let m = guard.msg_bye_ack(req.id().to_string(), req.payload().to_string());
                                    _ = c_bcast.send(m.sendable().unwrap());
                                });
                            }

                            RequestType::Event => {
                                log::debug!("Event for {}: {}", req.id(), req.payload());
                                let d = req.get_data();
                                let c_master = Arc::clone(&master);
                                tokio::spawn(async move {
                                    let m = c_master.lock().await;
                                    m.taskreg.lock().await.deregister(d.cid(), req.id());
                                    let mrec = m.mreg.lock().await.get(req.id()).unwrap_or_default().unwrap_or_default();

                                    // XXX: Fix this nonsense
                                    //      This should use get_data() method to extract payload properly
                                    //      Also replace HashMap in evtipc.add_event with it.
                                    let pl = match serde_json::from_str::<HashMap<String, serde_json::Value>>(req.payload().to_string().as_str()) {
                                        Ok(pl) => pl,
                                        Err(err) => {
                                            log::error!("An event message with the bogus payload: {err}");
                                            return;
                                        }
                                    };

                                    if m.cfg().telemetry_enabled() {
                                        // Sent OTEL log entry
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
                                    let sid = match master
                                        .evtipc
                                        .get_session(&util::dataconv::as_str(pl.get(&ProtoKey::CycleId.to_string()).cloned()))
                                        .await
                                    {
                                        Ok(sid) => sid,
                                        Err(err) => {
                                            log::debug!("Unable to acquire session for this iteration: {err}");
                                            return;
                                        }
                                    };

                                    if master.cfg().telemetry_enabled() {
                                        let mut otel = OtelLogger::new(&pl);
                                        otel.set_map(true); // Use mapper (only)
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
                                    _ = c_bcast.send(guard.msg_sensors_files().sendable().unwrap());
                                });
                            }

                            _ => {
                                log::error!("Minion sends unknown request type");
                            }
                        }
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
            let mid_short = if mid.chars().count() > 8 {
                format!("{}...{}", &mid[..4], &mid[mid.len() - 4..])
            } else {
                mid.to_string()
            };
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
            out.push(format!(
                "{}  {}  {}",
                pad_visible(&host, host_width),
                pad_visible(&ip, ip_width),
                pad_visible(&mid, id_width),
            ));
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
            self.mreg
                .lock()
                .await
                .get_by_traits(traits)?
        } else {
            self.mreg.lock().await.get_by_query(if query.trim().is_empty() { "*" } else { query })?
        };
        records.sort_by(|a, b| a.id().cmp(b.id()));
        Ok(records)
    }

    /// Execute one profile console request and return its console response plus any outbound master messages to broadcast.
    async fn profile_console_response(
        &mut self, request: &ProfileConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<(ConsoleResponse, Vec<MasterMessage>), SysinspectError> {
        fn require_profile_name(request: &ProfileConsoleRequest) -> Result<(), SysinspectError> {
            if !request.name().trim().is_empty() {
                return Ok(());
            }

            Err(SysinspectError::InvalidQuery("Profile name cannot be empty".to_string()))
        }

        let repo = SysInspectModPak::new(self.cfg.get_mod_repo_root())?;

        match request.op() {
            "new" => Ok((
                {
                    require_profile_name(request)?;
                    repo.new_profile(request.name())?;
                    ConsoleResponse { ok: true, message: format!("Created profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "delete" => Ok((
                {
                    require_profile_name(request)?;
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
                    require_profile_name(request)?;
                    ConsoleResponse { ok: true, message: repo.show_profile(request.name())? }
                },
                vec![],
            )),
            "add" => Ok((
                {
                    require_profile_name(request)?;
                    repo.add_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse { ok: true, message: format!("Updated profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "remove" => Ok((
                {
                    require_profile_name(request)?;
                    repo.remove_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse { ok: true, message: format!("Updated profile {}", request.name().bright_yellow()) }
                },
                vec![],
            )),
            "tag" | "untag" => {
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
                    let mut profiles = match minion.get_traits().get("minion.profile") {
                        Some(serde_json::Value::String(name)) if !name.trim().is_empty() => vec![name.to_string()],
                        Some(serde_json::Value::Array(names)) => names.iter().filter_map(|name| name.as_str().map(str::to_string)).collect::<Vec<_>>(),
                        _ => vec![],
                    };
                    profiles.retain(|profile| profile != "default");
                    if request.op() == "tag" {
                        for profile in request.profiles() {
                            if !profiles.contains(profile) {
                                profiles.push(profile.to_string());
                            }
                        }
                    } else {
                        profiles.retain(|profile| !request.profiles().contains(profile));
                    }
                    if let Some(msg) = self
                        .msg_query_data(
                            &format!("{SCHEME_COMMAND}{CLUSTER_TRAITS_UPDATE}"),
                            "",
                            "",
                            minion.id(),
                            &if profiles.is_empty() {
                                json!({"op": "unset", "traits": {"minion.profile": null}})
                            } else {
                                json!({"op": "set", "traits": {"minion.profile": profiles}})
                            }
                            .to_string(),
                        )
                        .await
                    {
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
            _ => Ok((
                ConsoleResponse { ok: false, message: format!("Unsupported profile operation {}", request.op().bright_yellow()) },
                vec![],
            )),
        }
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
                                                                    } else if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}")) {
                                                                        let (response, msgs) = match ProfileConsoleRequest::from_context(&query.context) {
                                                                            Ok(request) => {
                                                                                let mut guard = master.lock().await;
                                                                                match guard.profile_console_response(&request, &query.query, &query.traits, &query.mid).await {
                                                                                    Ok(data) => data,
                                                                                    Err(err) => (ConsoleResponse { ok: false, message: err.to_string() }, vec![]),
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
                                                                            let ids = guard.mreg.lock().await.get_targeted_minions(msg.target(), false).await;
                                                                            guard.taskreg.lock().await.register(msg.cycle(), ids);
                                                                        }
                                                                        response
                                                                    } else {
                                                                        let msg = {
                                                                            let mut guard = master.lock().await;
                                                                            guard.msg_query_data(&query.model, &query.query, &query.traits, &query.mid, &query.context).await
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
                                                                            let ids = guard.mreg.lock().await.get_targeted_minions(msg.target(), false).await;
                                                                            guard.taskreg.lock().await.register(msg.cycle(), ids);
                                                                            ConsoleResponse { ok: true, message: format!("Accepted console command from {peer}") }
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
                                            Err(err) => {
                                                serde_json::to_string(&ConsoleResponse {
                                                    ok: false,
                                                    message: format!("Failed to parse console request: {err}"),
                                                })
                                                .ok()
                                            }
                                        },
                                        Err(err) => {
                                            serde_json::to_string(&ConsoleResponse { ok: false, message: format!("Console request is not valid UTF-8: {err}") }).ok()
                                        }
                                    },
                                    Ok(Err(err)) => serde_json::to_string(&ConsoleResponse { ok: false, message: format!("Failed to read console request: {err}") }).ok(),
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
    pub async fn bcast_master_msg(
        bcast: &broadcast::Sender<Vec<u8>>, use_telemetry: bool, master: Arc<Mutex<SysMaster>>, msg: Option<MasterMessage>,
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

        let _ = bcast.send(msg.sendable().unwrap());
        log::debug!("Message broadcasted: {}", msg.target().scheme());
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
                        let peer_addr = socket.peer_addr().unwrap();
                        let (reader, writer) = socket.into_split();
                        let c_master = Arc::clone(&master);
                        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

                        // Task to send messages to the client
                        tokio::spawn(async move {
                            let mut writer = writer;
                            log::info!("Minion {peer_addr} connected. Ready to send messages.");

                            loop {
                                if let Ok(msg) = bcast_sub.recv().await {
                                    log::trace!("Sending message to minion at {} length of {}", peer_addr, msg.len());
                                    let mut guard = c_master.lock().await;
                                    if writer.write_all(&(msg.len() as u32).to_be_bytes()).await.is_err()
                                        || writer.write_all(&msg).await.is_err()
                                        || writer.flush().await.is_err()
                                    {
                                        if let Err(err) = cancel_tx.send(true) {
                                            log::debug!("Error sending cancel notification: {err}");
                                        }
                                        break;
                                    }

                                    if guard.to_drop.contains(&peer_addr.to_string()) {
                                        guard.to_drop.remove(&peer_addr.to_string());
                                        log::info!("Dropping minion: {}", &peer_addr.to_string());
                                        if let Err(err) = writer.shutdown().await {
                                            log::error!("Error shutting down outgoing: {err}");
                                        }
                                        if let Err(err) = cancel_tx.send(true) {
                                            log::debug!("Error sending cancel notification: {err}");
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
                                    let _ = client_tx.send((Vec::new(), peer_addr.to_string())).await;
                                    return;
                                }

                                let msg_len = u32::from_be_bytes(len_buf) as usize;
                                let mut msg = vec![0u8; msg_len];
                                if reader.read_exact(&mut msg).await.is_err() {
                                    let _ = client_tx.send((Vec::new(), peer_addr.to_string())).await;
                                    return;
                                }

                                if client_tx.send((msg, peer_addr.to_string())).await.is_err() {
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
