use crate::{
    config::{self},
    proto,
};
use libsysinspect::{
    rsa,
    util::{self},
    SysinspectError,
};
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::mpsc,
};

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
        log::debug!("To master: {}", String::from_utf8_lossy(&msg));
    }
}

/// Minion routine
pub async fn minion(mut cfp: PathBuf, fingerprint: Option<String>) -> Result<(), SysinspectError> {
    let fingerprint = fingerprint.unwrap_or_default();
    if !cfp.exists() {
        cfp = util::cfg::select_config()?;
    }
    let cfg = config::MinionConfig::new(cfp)?;
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

            match proto::msg::payload_to_msg(msg) {
                Ok(msg) => {
                    log::debug!("Received: {:?}", msg);
                    match msg.req_type() {
                        libsysinspect::proto::rqtypes::RequestType::Add => {
                            log::debug!("Master accepts registration");
                        }

                        libsysinspect::proto::rqtypes::RequestType::Reconnect => {
                            log::debug!("Master requires reconnection");
                            log::info!("{}", msg.payload());
                            std::process::exit(0);
                        }

                        libsysinspect::proto::rqtypes::RequestType::Remove => {
                            log::debug!("Master asks to unregister");
                        }
                        libsysinspect::proto::rqtypes::RequestType::Command => {
                            log::debug!("Master sends a command");
                        }
                        libsysinspect::proto::rqtypes::RequestType::Traits => {
                            log::debug!("Master requests traits");
                        }
                        libsysinspect::proto::rqtypes::RequestType::AgentUnknown => {
                            let pbk_pem = msg.payload(); // Expected PEM RSA pub key
                            let (_, pbk) = rsa::keys::from_pem(None, Some(pbk_pem)).unwrap();
                            let fpt = rsa::keys::get_fingerprint(&pbk.unwrap()).unwrap();

                            log::error!("Minion is not registered");
                            log::info!("Master fingerprint: {}", fpt);
                            std::process::exit(1);
                        }
                        _ => {
                            log::error!("Unknown request type");
                        }
                    }

                    //request(wtsm_c.clone(), response).await;
                }
                Err(err) => {
                    log::error!("{err}");
                }
            }
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
