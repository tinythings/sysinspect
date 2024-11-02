use libsysinspect::SysinspectError;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

pub(crate) async fn send_message(msg: &str, fifo: &str) -> Result<(), SysinspectError> {
    OpenOptions::new().write(true).open(fifo).await?.write_all(format!("{}\n", msg).as_bytes()).await?;
    log::debug!("Message sent to FIFO: {}", msg);
    Ok(())
}
