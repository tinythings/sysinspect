use crate::{config, traits};
use libsysinspect::{util, SysinspectError};
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::mpsc,
};

/// Talk-back to the master
pub async fn master_feedback(stream: Arc<Mutex<OwnedWriteHalf>>, msg: Vec<u8>) {
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
pub async fn minion(mut cfp: PathBuf) -> Result<(), SysinspectError> {
    if !cfp.exists() {
        cfp = util::cfg::select_config()?;
    }
    let cfg = config::MinionConfig::new(cfp)?;
    let st = traits::get_traits();

    let (rstm, wstm) = TcpStream::connect(cfg.master()).await?.into_split();
    let wstm = Arc::new(Mutex::new(wstm));
    let (_w_chan, mut r_chan) = mpsc::channel(100);

    // ehlo
    tokio::spawn(master_feedback(wstm.clone(), format!("Connected to {}", cfg.master()).as_bytes().to_vec()));

    // Data exchange
    let wtsm_c = wstm.clone();
    tokio::spawn(async move {
        let mut input = BufReader::new(rstm);
        loop {
            let mut buff = [0u8; 4];
            if let Err(e) = input.read_exact(&mut buff).await {
                log::error!("Unknown message length from the master: {}", e);
                break;
            }
            let msg_len = u32::from_be_bytes(buff) as usize;

            let mut msg = vec![0u8; msg_len];
            if let Err(e) = input.read_exact(&mut msg).await {
                log::error!("Invalid message from the master: {}", e);
                break;
            }

            log::info!("Received: {}", String::from_utf8_lossy(&msg));

            // Send a response back to the master after receiving each message
            let response = format!("Back: '{}'", String::from_utf8_lossy(&msg)).as_bytes().to_vec();
            master_feedback(wtsm_c.clone(), response).await;
        }
    });

    // Task to handle queued messages for the master
    let qmsg_stm = wstm.to_owned();
    tokio::spawn(async move {
        while let Some(msg) = r_chan.recv().await {
            master_feedback(qmsg_stm.clone(), msg).await;
        }
    });

    // Keep the client alive until Ctrl+C is pressed
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Shutting down client.");
    Ok(())
}
