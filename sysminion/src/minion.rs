use crate::{arcb::ActionResponseCallback, filedata::MinionFiledata, proto, rsa::MinionRSAKeyManager};
use clap::ArgMatches;
use colored::Colorize;
use libsetup::get_ssh_client_ip;
use libsysinspect::{
    SysinspectError,
    cfg::{
        get_minion_config,
        mmconf::{DEFAULT_PORT, MinionConfig, SysInspectConfig},
    },
    inspector::SysInspectRunner,
    intp::actproc::response::ActionResponse,
    proto::{
        MasterMessage, MinionMessage, ProtoConversion,
        errcodes::ProtoErrorCode,
        payload::{ModStatePayload, PayloadType},
        query::{MinionQuery, SCHEME_COMMAND},
        rqtypes::RequestType,
    },
    rsa,
    traits::{self},
    util::{self, dataconv},
};
use once_cell::sync::Lazy;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
    vec,
};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, tcp::OwnedReadHalf};
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use uuid::Uuid;

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());
#[derive(Debug)]
pub struct SysMinion {
    cfg: MinionConfig,
    fingerprint: Option<String>,
    kman: MinionRSAKeyManager,

    rstm: Arc<Mutex<OwnedReadHalf>>,
    wstm: Arc<Mutex<OwnedWriteHalf>>,

    filedata: Mutex<MinionFiledata>,

    last_ping: Mutex<Instant>,
    ping_timeout: Duration,
}

impl SysMinion {
    pub async fn new(cfg: MinionConfig, fingerprint: Option<String>) -> Result<Arc<SysMinion>, SysinspectError> {
        log::debug!("Configuration: {:#?}", cfg);
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
            log::debug!(
                "Creating directory for the models at {}",
                self.cfg.models_dir().as_os_str().to_str().unwrap_or_default()
            );
            fs::create_dir_all(self.cfg.models_dir())?;
        }

        // Place for traits.d
        if !self.cfg.traits_dir().exists() {
            log::debug!(
                "Creating directory for the drop-in traits at {}",
                self.cfg.traits_dir().as_os_str().to_str().unwrap_or_default()
            );
            fs::create_dir_all(self.cfg.traits_dir())?;
        }

        // Place for trait functions
        if !self.cfg.functions_dir().exists() {
            log::debug!(
                "Creating directory for the custom trait functions at {}",
                self.cfg.functions_dir().as_os_str().to_str().unwrap_or_default()
            );
            fs::create_dir_all(self.cfg.functions_dir())?;
        }

        let mut out: Vec<String> = vec![];
        for t in traits::get_minion_traits(Some(&self.cfg)).items() {
            out.push(format!(
                "{}: {}",
                t.to_owned(),
                dataconv::to_string(traits::get_minion_traits(None).get(&t)).unwrap_or_default()
            ));
        }
        log::debug!("Minion traits:\n{}", out.join("\n"));

        Ok(())
    }

    async fn update_ping(&self) {
        let mut last_ping = self.last_ping.lock().await;
        *last_ping = Instant::now();
    }

    pub fn as_ptr(self: &Arc<Self>) -> Arc<Self> {
        Arc::clone(self)
    }

    /// Get current minion Id
    fn get_minion_id(&self) -> String {
        dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID))
    }

    /// Talk-back to the master
    async fn request(&self, msg: Vec<u8>) {
        let mut stm = self.wstm.lock().await;

        if let Err(e) = stm.write_all(&(msg.len() as u32).to_be_bytes()).await {
            log::error!("Failed to send message length to master: {}", e);
            return;
        }

        if let Err(e) = stm.write_all(&msg).await {
            log::error!("Failed to send message to master: {}", e);
            return;
        }

        if let Err(e) = stm.flush().await {
            log::error!("Failed to flush writer to master: {}", e);
        } else {
            log::trace!("To master: {}", String::from_utf8_lossy(&msg));
        }
    }

    /// A sub-process that checks if a ping is going through. On ping timeout
    /// that would indicate that the Master is either dead or disconnected or not available.
    /// That should kick Minion to start reconnecting.
    pub async fn do_ping_update(self: Arc<Self>) -> Result<(), SysinspectError> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if self.as_ptr().last_ping.lock().await.elapsed() > self.as_ptr().ping_timeout {
                    log::warn!("Master seems unresponsive, terminating.");
                    std::process::exit(1);
                }
            }
        });
        Ok(())
    }

    pub async fn do_proto(self: Arc<Self>) -> Result<(), SysinspectError> {
        let rstm = Arc::clone(&self.rstm);

        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 4];
                if let Err(e) = rstm.lock().await.read_exact(&mut buff).await {
                    log::trace!("Unknown message length from the master: {}", e);
                    break;
                }
                let msg_len = u32::from_be_bytes(buff) as usize;

                let mut msg = vec![0u8; msg_len];
                if let Err(e) = rstm.lock().await.read_exact(&mut msg).await {
                    log::error!("Invalid message from the master: {}", e);
                    break;
                }

                let msg = match proto::msg::payload_to_msg(msg) {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::error!("Error getting network payload as message: {err}");
                        continue;
                    }
                };

                log::trace!("Received: {:#?}", msg);

                match msg.req_type() {
                    RequestType::Add => {
                        log::debug!("Master accepts registration");
                    }

                    RequestType::Reconnect => {
                        log::debug!("Master requires reconnection");
                        log::info!("{}", msg.payload());
                        std::process::exit(0);
                    }

                    RequestType::Remove => {
                        log::debug!("Master asks to unregister");
                    }
                    RequestType::Command => {
                        log::debug!("Master sends a command");
                        match msg.get_retcode() {
                            ProtoErrorCode::Success => {
                                let cls = self.as_ptr().clone();
                                tokio::spawn(async move {
                                    cls.dispatch(msg.to_owned()).await;
                                });
                            }
                            ProtoErrorCode::AlreadyConnected => {
                                if MINION_SID.eq(msg.payload()) {
                                    log::error!("Another minion from this machine is already connected");
                                    std::process::exit(1);
                                }
                            }
                            ret => {
                                log::debug!("Return code {:?} not yet implemented", ret);
                            }
                        }
                    }
                    RequestType::Traits => {
                        log::debug!("Master requests traits");
                        if let Err(err) = self.as_ptr().send_traits().await {
                            log::error!("Unable to send traits: {err}");
                        }
                    }
                    RequestType::AgentUnknown => {
                        let pbk_pem = dataconv::as_str(Some(msg.payload()).cloned()); // Expected PEM RSA pub key
                        let (_, pbk) = rsa::keys::from_pem(None, Some(&pbk_pem)).unwrap();
                        let fpt = rsa::keys::get_fingerprint(&pbk.unwrap()).unwrap();

                        log::error!("Minion is not registered");
                        log::info!("Master fingerprint: {}", fpt);
                        std::process::exit(1);
                    }
                    RequestType::Ping => {
                        self.request(proto::msg::get_pong()).await;
                        self.update_ping().await;
                    }
                    RequestType::ByeAck => {
                        log::info!("Master confirmed shutdown, terminating");
                        std::process::exit(0);
                    }
                    _ => {
                        log::error!("Unknown request type");
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn send_traits(self: Arc<Self>) -> Result<(), SysinspectError> {
        let mut r =
            MinionMessage::new(self.get_minion_id(), RequestType::Traits, traits::get_minion_traits(None).to_json_string()?);
        r.set_sid(MINION_SID.to_string());
        self.request(r.sendable().unwrap()).await; // XXX: make a better error handling for Tokio
        Ok(())
    }

    /// Send ehlo
    pub async fn send_ehlo(self: Arc<Self>) -> Result<(), SysinspectError> {
        let mut r = MinionMessage::new(
            dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)),
            RequestType::Ehlo,
            MINION_SID.to_string(),
        );
        r.set_sid(MINION_SID.to_string());

        log::info!("Ehlo on {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send registration request
    pub async fn send_registration(self: Arc<Self>, pbk_pem: String) -> Result<(), SysinspectError> {
        let r =
            MinionMessage::new(dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)), RequestType::Add, pbk_pem);

        log::info!("Registration request to {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send callback to the master on the results
    pub async fn send_callback(self: Arc<Self>, ar: ActionResponse) -> Result<(), SysinspectError> {
        log::info!("Sending sync callback on {}", ar.aid());
        self.request(MinionMessage::new(self.get_minion_id(), RequestType::Event, json!(ar).to_string()).sendable()?).await;
        Ok(())
    }

    /// Send bye message
    pub async fn send_bye(self: Arc<Self>) {
        let r = MinionMessage::new(
            dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)),
            RequestType::Bye,
            MINION_SID.to_string(),
        );

        log::info!("Goodbye to {}", self.cfg.master());
        self.request(r.sendable().unwrap()).await;
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
                        return Err(SysinspectError::MinionGeneralError(format!("Unable to get text from the file: {}", err)));
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
    async fn launch_sysinspect(self: Arc<Self>, cycle_id: &str, scheme: &str, msp: &ModStatePayload) {
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
            log::debug!("File {uri_file} has different checksum");

            match self.as_ptr().download_file(uri_file).await {
                Ok(data) => {
                    let dst_dir = dst.parent().unwrap();
                    if !dst_dir.exists() {
                        log::debug!("Creating directory: {:?}", dst_dir);
                        if let Err(err) = fs::create_dir_all(dst_dir) {
                            log::error!("Unable to create directories for model download: {err}");
                            return;
                        }
                    }

                    log::debug!("Saving URI {uri_file} as {:?}", dst_dir);
                    if let Err(err) = fs::write(&dst, data) {
                        log::error!("Unable to save downloaded file to {:?}: {err}", dst);
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

        // Render DSL

        // Run the model
        log::debug!("Launching model for sysinspect for: {scheme}");
        let mqr_l = mqr.lock().await;

        let mut sr = SysInspectRunner::new(&self.cfg);
        sr.set_model_path(self.as_ptr().cfg.models_dir().join(mqr_l.target()).to_str().unwrap_or_default());
        sr.set_state(mqr_l.state());
        sr.set_entities(mqr_l.entities());
        sr.set_checkbook_labels(mqr_l.checkbook_labels());
        sr.set_traits(traits::get_minion_traits(None));

        sr.add_async_callback(Box::new(ActionResponseCallback::new(self.as_ptr(), cycle_id)));

        sr.start().await;

        log::debug!("Sysinspect model cycle finished");
    }

    /// Calls internal command
    async fn call_internal_command(self: Arc<Self>, cmd: &str) {
        let cmd = cmd.strip_prefix(SCHEME_COMMAND).unwrap_or_default();
        match cmd {
            libsysinspect::proto::query::commands::CLUSTER_SHUTDOWN => {
                log::info!("Requesting minion shutdown from a master");
                self.as_ptr().send_bye().await;
            }
            libsysinspect::proto::query::commands::CLUSTER_REBOOT => {
                log::warn!("Command \"reboot\" is not implemented yet");
            }
            libsysinspect::proto::query::commands::CLUSTER_ROTATE => {
                log::warn!("Command \"rotate\" is not implemented yet");
            }
            libsysinspect::proto::query::commands::CLUSTER_REMOVE_MINION => {
                log::info!("{} from the master", "Unregistering".bright_red().bold());
                self.as_ptr().send_bye().await;
            }
            _ => {
                log::warn!("Unknown command: {cmd}");
            }
        }
    }

    async fn dispatch(self: Arc<Self>, cmd: MasterMessage) {
        log::debug!("Dispatching message: {:#?}", cmd);

        if cmd.get_cycle().is_empty() {
            log::error!("Cycle ID is empty!");
            return;
        }

        let tgt = cmd.get_target();

        // Is command minion-specific?
        if !tgt.id().is_empty() && tgt.id().ne(&self.get_minion_id()) {
            log::trace!("Command was dropped as it was specifically addressed for another minion");
            return;
        } else if tgt.id().is_empty() {
            let traits = traits::get_minion_traits(None);

            // Is matching this host?
            let mut skip = true;
            let hostname = dataconv::as_str(traits.get("system.hostname"));
            if !hostname.is_empty() {
                for hq in tgt.hostnames() {
                    if let Ok(hq) = glob::Pattern::new(hq) {
                        if hq.matches(&hostname) {
                            skip = false;
                            break;
                        }
                    }
                }
                if skip {
                    log::trace!("Command was dropped as it is specifically targeting different hosts");
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
                                    log::trace!("Command was dropped as it does not match the traits");
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

        match PayloadType::try_from(cmd.payload().clone()) {
            Ok(PayloadType::ModelOrStatement(pld)) => {
                if cmd.get_target().scheme().starts_with(SCHEME_COMMAND) {
                    self.as_ptr().call_internal_command(cmd.get_target().scheme()).await;
                } else {
                    self.as_ptr().launch_sysinspect(cmd.get_cycle(), cmd.get_target().scheme(), &pld).await;
                    log::debug!("Command dispatched");
                    log::debug!("Command payload: {:#?}", pld);
                }
            }
            Ok(PayloadType::Undef(pld)) => {
                log::error!("Unknown command: {:#?}", pld);
            }
            Err(err) => {
                log::error!("Error dispatching command: {err}");
            }
        }
    }
}

pub async fn minion(cfg: MinionConfig, fingerprint: Option<String>) -> Result<(), SysinspectError> {
    let minion = SysMinion::new(cfg, fingerprint).await?;
    minion.as_ptr().do_proto().await?;

    // Messages
    if minion.fingerprint.is_some() {
        minion.as_ptr().send_registration(minion.kman.get_pubkey_pem()).await?;
    } else {
        // ehlo
        minion.as_ptr().send_ehlo().await?;
    }

    minion.as_ptr().do_ping_update().await?;

    // Keep the client alive until Ctrl+C is pressed
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Shutting down client.");

    Ok(())
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

    libsetup::mnsetup::MinionSetup::new()
        .set_config(get_minion_config(None)?)
        .set_alt_dir(dir.to_str().unwrap_or_default().to_string())
        .setup()
}
