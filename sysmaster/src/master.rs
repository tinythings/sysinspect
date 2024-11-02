use crate::config::MasterConfig;
use libsysinspect::SysinspectError;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};

/// Open FIFO socket for command-line communication
fn open_socket(path: &str) -> Result<(), SysinspectError> {
    if !Path::new(path).exists() {
        if unsafe { libc::mkfifo(std::ffi::CString::new(path)?.as_ptr(), 0o600) } != 0 {
            return Err(SysinspectError::ConfigError(format!("{}", std::io::Error::last_os_error())));
        }
        log::info!("Socket opened at {}", path);
    }
    Ok(())
}

pub(crate) async fn master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    log::info!("Starting master at {}", cfg.bind_addr());
    open_socket(&cfg.socket())?;

    let listener = TcpListener::bind(cfg.bind_addr()).await?;
    let (tx, _) = broadcast::channel::<Vec<u8>>(100);
    let (client_tx, mut client_rx) = mpsc::channel::<(Vec<u8>, usize)>(100);

    // Task to read from the FIFO and broadcast messages to clients
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        loop {
            match OpenOptions::new().read(true).open(cfg.socket()).await {
                Ok(file) => {
                    let reader = TokioBufReader::new(file);
                    let mut lines = reader.lines();

                    loop {
                        select! {
                            line = lines.next_line() => {
                                match line {
                                    Ok(Some(message)) => {
                                        log::info!("Broadcasting FIFO message to clients: {}", message);
                                        let _ = tx_clone.send(message.into_bytes());
                                    }
                                    Ok(None) => break, // End of file, re-open the FIFO
                                    Err(e) => {
                                        log::error!("Error reading from FIFO: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to open FIFO: {}", e);
                    sleep(Duration::from_secs(1)).await; // Retry after a sec
                }
            }
        }
    });

    // Handle incoming messages from minions
    tokio::spawn(async move {
        loop {
            if let Some((msg, client_id)) = client_rx.recv().await {
                log::info!("Minion: {}: {}", client_id, String::from_utf8_lossy(&msg));
            } else {
                break;
            }
        }
    });

    // Accept connections and spawn tasks for each client
    tokio::spawn(async move {
        let mut client_id_counter: usize = 0;
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                client_id_counter += 1;
                let current_client_id = client_id_counter;
                let mut rx = tx.subscribe();
                let client_tx = client_tx.clone();

                let (reader, writer) = socket.into_split();

                // Task to send messages to the client
                tokio::spawn(async move {
                    let mut writer = writer;
                    log::info!("Minion {} connected. Ready to send messages.", current_client_id);
                    loop {
                        if let Ok(msg) = rx.recv().await {
                            log::info!("Sending message to client {}: {:?}", current_client_id, msg);
                            if writer.write_all(&(msg.len() as u32).to_be_bytes()).await.is_err()
                                || writer.write_all(&msg).await.is_err()
                                || writer.flush().await.is_err()
                            {
                                break;
                            }
                        }
                    }
                });

                // Task to read messages from the client
                tokio::spawn(async move {
                    let mut reader = TokioBufReader::new(reader);
                    loop {
                        let mut len_buf = [0u8; 4];
                        if reader.read_exact(&mut len_buf).await.is_err() {
                            return;
                        }

                        let msg_len = u32::from_be_bytes(len_buf) as usize;
                        let mut msg = vec![0u8; msg_len];
                        if reader.read_exact(&mut msg).await.is_err() {
                            return;
                        }

                        if client_tx.send((msg, current_client_id)).await.is_err() {
                            break;
                        }
                    }
                });
            }
        }
    });

    // Listen for shutdown signal and cancel tasks
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    log::info!("Received shutdown signal.");
    std::process::exit(0);
}
