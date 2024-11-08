use crate::proto;
use libsysinspect::{
    cfg::{self, mmconf::MinionConfig},
    proto::{errcodes::ProtoErrorCode, rqtypes::RequestType},
    rsa, SysinspectError,
};
use once_cell::sync::Lazy;
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::mpsc,
};
use uuid::Uuid;

/// Session Id of the minion
pub static MINION_SID: Lazy<String> = Lazy::new(|| Uuid::new_v4().to_string());

/// Talk-back to the master
pub async fn request(stream: Arc<Mutex<OwnedWriteHalf>>, msg: Vec<u8>) {
    let mut stm = stream.lock().await;

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

/// Minion routine
pub async fn minion(mut cfp: PathBuf, fingerprint: Option<String>) -> Result<(), SysinspectError> {
    let fingerprint = fingerprint.unwrap_or_default();
    if !cfp.exists() {
        cfp = cfg::select_config(None)?;
    }
    let cfg = MinionConfig::new(cfp)?;
    //let st = traits::get_traits();
    let mkeys = crate::rsa::MinionRSAKeyManager::new(None)?; // XXX: Get optional root from the configuration

    let (rstm, wstm) = TcpStream::connect(cfg.master()).await?.into_split();
    let wstm: Arc<Mutex<OwnedWriteHalf>> = Arc::new(Mutex::new(wstm));
    let (_w_chan, mut r_chan) = mpsc::channel(100);

    // Data exchange
    let wtsm_c = wstm.clone();
    tokio::spawn(async move {
        let mut input = BufReader::new(rstm);

        loop {
            let mut buff = [0u8; 4];
            if let Err(e) = input.read_exact(&mut buff).await {
                log::trace!("Unknown message length from the master: {}", e);
                break;
            }
            let msg_len = u32::from_be_bytes(buff) as usize;

            let mut msg = vec![0u8; msg_len];
            if let Err(e) = input.read_exact(&mut msg).await {
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
                    request(wtsm_c.clone(), proto::msg::get_pong()).await;
                }
                _ => {
                    log::error!("Unknown request type");
                }
            }

            //request(wtsm_c.clone(), response).await;
        }
    });

    // Task to handle queued messages for the master
    let qmsg_stm = wstm.to_owned();
    tokio::spawn(async move {
        while let Some(msg) = r_chan.recv().await {
            request(qmsg_stm.clone(), msg).await;
        }
    });

    // Messages
    if !fingerprint.is_empty() {
        proto::msg::send_registration(wstm.clone(), cfg, mkeys.get_pubkey_pem()).await?;
    } else {
        // ehlo
        proto::msg::send_ehlo(wstm.clone(), cfg).await?;
    }

    // Keep the client alive until Ctrl+C is pressed
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Shutting down client.");
    Ok(())
}
