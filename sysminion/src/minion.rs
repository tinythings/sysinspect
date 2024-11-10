use crate::{proto, rsa::MinionRSAKeyManager, traits};
use libsysinspect::{
    cfg::{self, mmconf::MinionConfig},
    proto::{errcodes::ProtoErrorCode, rqtypes::RequestType, MinionMessage, ProtoConversion},
    rsa,
    util::dataconv,
    SysinspectError,
};
use once_cell::sync::Lazy;
use std::{path::PathBuf, sync::Arc};
use tokio::net::{tcp::OwnedReadHalf, TcpStream};
use tokio::sync::Mutex;
use tokio::{io::AsyncReadExt, sync::mpsc};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use uuid::Uuid;

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());

pub struct SysMinion {
    cfg: MinionConfig,
    fingerprint: Option<String>,
    kman: MinionRSAKeyManager,

    rstm: Arc<Mutex<OwnedReadHalf>>,
    wstm: Arc<Mutex<OwnedWriteHalf>>,
}

impl SysMinion {
    pub async fn new(cfp: &str, fingerprint: Option<String>) -> Result<Arc<SysMinion>, SysinspectError> {
        let mut cfp = PathBuf::from(cfp);
        if !cfp.exists() {
            cfp = cfg::select_config(None)?;
        }

        let cfg = MinionConfig::new(cfp)?;
        let (rstm, wstm) = TcpStream::connect(cfg.master()).await.unwrap().into_split();
        Ok(Arc::new(SysMinion {
            cfg,
            fingerprint,
            kman: MinionRSAKeyManager::new(None)?,
            rstm: Arc::new(Mutex::new(rstm)),
            wstm: Arc::new(Mutex::new(wstm)),
        }))
    }

    pub fn as_ptr(self: &Arc<Self>) -> Arc<Self> {
        Arc::clone(self)
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

    pub async fn do_qmsg(self: Arc<Self>) {
        let (_w_chan, mut r_chan) = mpsc::channel(100);
        let cls = Arc::new(self.clone());
        tokio::spawn(async move {
            while let Some(msg) = r_chan.recv().await {
                cls.request(msg).await;
            }
        });
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

                log::trace!("Received: {:?}", msg);

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
                    }
                    RequestType::AgentUnknown => {
                        let pbk_pem = msg.payload(); // Expected PEM RSA pub key
                        let (_, pbk) = rsa::keys::from_pem(None, Some(pbk_pem)).unwrap();
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

    /// Send ehlo
    pub async fn send_ehlo(self: Arc<Self>) -> Result<(), SysinspectError> {
        let r = MinionMessage::new(
            dataconv::as_str(traits::get_traits().get(traits::SYS_ID.to_string())),
            RequestType::Ehlo,
            MINION_SID.to_string(),
        );

        log::info!("Ehlo on {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Send registration request
    pub async fn send_registration(self: Arc<Self>, pbk_pem: String) -> Result<(), SysinspectError> {
        let r =
            MinionMessage::new(dataconv::as_str(traits::get_traits().get(traits::SYS_ID.to_string())), RequestType::Add, pbk_pem);

        log::info!("Registration request to {}", self.cfg.master());
        self.request(r.sendable()?).await;
        Ok(())
    }

    /// Download a file from master
    async fn download_file(self: Arc<Self>, fname: String) {
        async fn fetch_file(url: &str, filename: &str) -> Result<String, SysinspectError> {
            let rsp = match reqwest::get(format!("http://{}/{}", url, filename)).await {
                Ok(rsp) => rsp,
                Err(err) => {
                    return Err(SysinspectError::MinionGeneralError(format!("{}", err)));
                }
            };

            Ok(match rsp.status() {
                reqwest::StatusCode::OK => match rsp.text().await {
                    Ok(data) => data,
                    Err(err) => {
                        return Err(SysinspectError::MinionGeneralError(format!("{}", err)));
                    }
                },
                reqwest::StatusCode::NOT_FOUND => return Err(SysinspectError::MinionGeneralError("File not found".to_string())),
                _ => return Err(SysinspectError::MinionGeneralError("Unknown status".to_string())),
            })
        }
        let addr = self.cfg.fileserver();
        tokio::spawn(async move {
            match fetch_file(&addr, &fname).await {
                Ok(data) => log::debug!("Result returned as {:#?}", data),
                Err(err) => log::error!("{err}"),
            }
        });
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
