pub mod msg {
    use crate::{
        config::MinionConfig,
        minion::{request, MINION_SID},
        traits,
    };
    use libsysinspect::{
        proto::{rqtypes::RequestType, MinionMessage},
        util::dataconv,
    };
    use libsysinspect::{
        proto::{MasterMessage, ProtoConversion},
        SysinspectError,
    };
    use std::sync::Arc;
    use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

    /// Make pong message
    pub fn get_pong() -> Vec<u8> {
        let p = MinionMessage::new(
            dataconv::as_str(traits::get_traits().get(traits::SYS_ID.to_string())),
            RequestType::Pong,
            MINION_SID.to_string(),
        );

        if let Ok(data) = p.sendable() {
            return data;
        }
        vec![]
    }

    /// Send ehlo
    pub async fn send_ehlo(stream: Arc<Mutex<OwnedWriteHalf>>, cfg: MinionConfig) -> Result<(), SysinspectError> {
        let r = MinionMessage::new(
            dataconv::as_str(traits::get_traits().get(traits::SYS_ID.to_string())),
            RequestType::Ehlo,
            MINION_SID.to_string(),
        );

        log::info!("Ehlo on {}", cfg.master());
        request(stream, r.sendable()?).await;
        Ok(())
    }

    pub async fn send_registration(
        stream: Arc<Mutex<OwnedWriteHalf>>, cfg: MinionConfig, pbk_pem: String,
    ) -> Result<(), SysinspectError> {
        let r =
            MinionMessage::new(dataconv::as_str(traits::get_traits().get(traits::SYS_ID.to_string())), RequestType::Add, pbk_pem);

        log::info!("Registration request to {}", cfg.master());
        request(stream, r.sendable()?).await;
        Ok(())
    }

    /// Get message
    pub fn payload_to_msg(data: Vec<u8>) -> Result<MasterMessage, SysinspectError> {
        let data = match String::from_utf8(data) {
            Ok(data) => data,
            Err(err) => return Err(SysinspectError::ProtoError(format!("unable to parse master message: {err}"))),
        };

        let msg = match serde_json::from_str::<MasterMessage>(&data) {
            Ok(msg) => msg,
            Err(err) => {
                log::trace!("Broken JSON message: {data}");
                return Err(SysinspectError::ProtoError(format!("broken JSON from master message: {err}")));
            }
        };

        Ok(msg)
    }
}
