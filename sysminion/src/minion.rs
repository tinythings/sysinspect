use crate::{filedata::MinionFiledata, proto, rsa::MinionRSAKeyManager};
use libsysinspect::{
    cfg::{self, mmconf::MinionConfig},
    proto::{
        errcodes::ProtoErrorCode,
        payload::{ModStatePayload, PayloadType},
        rqtypes::RequestType,
        MasterMessage, MinionMessage, ProtoConversion,
    },
    rsa,
    traits::{self, systraits::SystemTraits},
    util::dataconv,
    SysinspectError,
};
use once_cell::sync::{Lazy, OnceCell};
use std::{fs, path::PathBuf, sync::Arc, vec};
use tokio::io::AsyncReadExt;
use tokio::net::{tcp::OwnedReadHalf, TcpStream};
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use uuid::Uuid;

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());
/*
Traits are system properties and attributes on which a minion is running.

P.S. These are not Rust traits. :-)
 */

/// System traits instance
static _TRAITS: OnceCell<SystemTraits> = OnceCell::new();

/// Returns a copy of initialised traits.
pub fn get_minion_traits() -> SystemTraits {
    _TRAITS.get().expect("Traits are not initialised!?").to_owned()
}

pub struct SysMinion {
    cfg: MinionConfig,
    fingerprint: Option<String>,
    kman: MinionRSAKeyManager,

    rstm: Arc<Mutex<OwnedReadHalf>>,
    wstm: Arc<Mutex<OwnedWriteHalf>>,

    filedata: Mutex<MinionFiledata>,
}

impl SysMinion {
    pub async fn new(cfp: &str, fingerprint: Option<String>) -> Result<Arc<SysMinion>, SysinspectError> {
        let mut cfp = PathBuf::from(cfp);
        if !cfp.exists() {
            cfp = cfg::select_config(None)?;
        }

        let cfg = MinionConfig::new(cfp)?;

        // Init traits
        _TRAITS.get_or_init(|| SystemTraits::new(cfg.clone()));

        let (rstm, wstm) = TcpStream::connect(cfg.master()).await.unwrap().into_split();
        let instance = SysMinion {
            cfg: cfg.clone(),
            fingerprint,
            kman: MinionRSAKeyManager::new(cfg.root_dir())?,
            rstm: Arc::new(Mutex::new(rstm)),
            wstm: Arc::new(Mutex::new(wstm)),
            filedata: Mutex::new(MinionFiledata::new(cfg.models_dir())?),
        };
        instance.init()?;

        Ok(Arc::new(instance))
    }

    /// Initialise minion.
    /// This creates all directory structures if none etc.
    fn init(&self) -> Result<(), SysinspectError> {
        log::info!("Initialising minion");
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
        for t in get_minion_traits().items() {
            out.push(format!("{}: {}", t.to_owned(), dataconv::to_string(get_minion_traits().get(&t)).unwrap_or_default()));
        }
        log::debug!("Minion traits:\n{}", out.join("\n"));

        Ok(())
    }

    pub fn as_ptr(self: &Arc<Self>) -> Arc<Self> {
        Arc::clone(self)
    }

    /// Get current minion Id
    fn get_minion_id(&self) -> String {
        dataconv::as_str(get_minion_traits().get(traits::SYS_ID))
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

    pub async fn do_proto(self: Arc<Self>) -> Result<(), SysinspectError> {
        let rstm = Arc::clone(&self.rstm);
        let cls = Arc::new(self.clone());

        tokio::spawn(async move {
            //let mut input = BufReader::new(rstm.lock().await);
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
                                let cls = cls.as_ptr().clone();
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
                        if let Err(err) = cls.as_ptr().send_traits().await {
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
                        cls.request(proto::msg::get_pong()).await;
                    }
                    _ => {
                        log::error!("Unknown request type");
                    }
                }

                //request(wtsm_c.clone(), response).await;
            }
        });
        Ok(())
    }

    pub async fn send_traits(self: Arc<Self>) -> Result<(), SysinspectError> {
        let mut r = MinionMessage::new(self.get_minion_id(), RequestType::Traits, get_minion_traits().to_json_string()?);
        r.set_sid(MINION_SID.to_string());
        self.request(r.sendable().unwrap()).await; // XXX: make a better error handling for Tokio
        Ok(())
    }

    /// Send ehlo
    pub async fn send_ehlo(self: Arc<Self>) -> Result<(), SysinspectError> {
        let mut r = MinionMessage::new(
            dataconv::as_str(get_minion_traits().get(traits::SYS_ID)),
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
        let r = MinionMessage::new(dataconv::as_str(get_minion_traits().get(traits::SYS_ID)), RequestType::Add, pbk_pem);

        log::info!("Registration request to {}", self.cfg.master());
        self.request(r.sendable()?).await;
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
    async fn launch_sysinspect(self: Arc<Self>, msp: &ModStatePayload) {
        // TODO: Now dispatch sysinspect!
        //
        // 1. [x] Check if files are there, if not download them
        // 2. [ ] Render the DSL according to the traits
        // 3. [ ] Run the model
        // 4. [ ] Collect the output and send back

        // Check if all files are there.
        let cls = Arc::new(self);
        let mut dirty = false;
        for (uri_file, fcs) in msp.files() {
            let dst = cls
                .cfg
                .models_dir()
                .join(uri_file.trim_start_matches(&format!("/{}", msp.models_root())).strip_prefix("/").unwrap_or_default());

            if cls.as_ptr().filedata.lock().await.check_sha256(uri_file.to_owned(), fcs.to_owned(), true) {
                continue;
            }
            log::debug!("File {uri_file} has different checksum");

            match cls.as_ptr().download_file(uri_file).await {
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
            cls.as_ptr().filedata.lock().await.init();
        }

        log::debug!("Launching model for sysinspect");
        // TODO: launch sysinspect here
    }

    async fn dispatch(self: Arc<Self>, cmd: MasterMessage) {
        log::debug!("Dispatching message");
        let tgt = cmd.get_target();

        // Is command minion-specific?
        if !tgt.id().is_empty() && tgt.id().ne(&self.get_minion_id()) {
            log::trace!("Command was dropped as it was specifically addressed for another minion");
            return;
        } else if tgt.id().is_empty() {
            let traits = get_minion_traits();

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
                                if !traits::matches_traits(tpq, get_minion_traits()) {
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

        match PayloadType::try_from(cmd.payload().clone()) {
            Ok(PayloadType::ModelOrStatement(pld)) => {
                self.launch_sysinspect(&pld).await;
                log::debug!("Command dispatched");
                log::trace!("Command payload: {:#?}", pld);
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

pub async fn minion(cfp: &str, fingerprint: Option<String>) -> Result<(), SysinspectError> {
    let minion = SysMinion::new(cfp, fingerprint).await?;
    minion.as_ptr().do_proto().await?;

    // Example downloading file
    //minion.as_ptr().download_file("models/inherited/model.cfg".to_string()).await;

    // Messages
    if minion.fingerprint.is_some() {
        minion.as_ptr().send_registration(minion.kman.get_pubkey_pem()).await?;
    } else {
        // ehlo
        minion.as_ptr().send_ehlo().await?;
    }

    // Keep the client alive until Ctrl+C is pressed
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Shutting down client.");

    Ok(())
}
