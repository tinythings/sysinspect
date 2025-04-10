pub mod msg {
    use std::sync::atomic::AtomicBool;

    use crate::minion::MINION_SID;
    use libsysinspect::{
        SysinspectError,
        proto::{MasterMessage, ProtoConversion},
    };
    use libsysinspect::{
        proto::{MinionMessage, rqtypes::RequestType},
        traits,
        util::dataconv,
    };
    use once_cell::sync::Lazy;
    use tokio::sync::broadcast;

    /// Channel for master connection status
    pub static CONNECTION_TX: Lazy<broadcast::Sender<()>> = Lazy::new(|| {
        let (tx, _) = broadcast::channel(1); // We have a small but enough buffer
        tx
    });

    pub struct ExitState {
        pub exit: AtomicBool,
    }

    impl ExitState {
        pub fn new() -> Self {
            Self { exit: AtomicBool::new(false) }
        }
    }

    /// Make pong message
    pub fn get_pong() -> Vec<u8> {
        let p = MinionMessage::new(
            dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)),
            RequestType::Pong,
            MINION_SID.to_string(),
        );

        if let Ok(data) = p.sendable() {
            return data;
        }
        vec![]
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
