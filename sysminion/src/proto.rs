pub mod msg {
    use crate::minion::MINION_SID;
    use libcommon::SysinspectError;
    use libsysinspect::{traits, util::dataconv};
    use libsysproto::{
        MasterMessage, MinionMessage, ProtoConversion,
        rqtypes::{ProtoKey, ProtoValue, RequestType},
    };
    use once_cell::sync::Lazy;
    use serde_json::{Value, json, to_value};
    use std::{collections::HashMap, sync::atomic::AtomicBool};
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
    pub fn get_pong(t: ProtoValue, payload: Option<Value>) -> Vec<u8> {
        let mut data: HashMap<String, Value> = HashMap::new();
        data.insert(ProtoKey::ProtoType.to_string(), to_value(t).unwrap());
        data.insert(ProtoKey::SessionId.to_string(), to_value(MINION_SID.to_string()).unwrap());
        data.insert(
            ProtoKey::Payload.to_string(),
            match payload {
                Some(pl) => pl,
                None => json!({}),
            },
        );

        let p = MinionMessage::new(dataconv::as_str(traits::get_minion_traits(None).get(traits::SYS_ID)), RequestType::Pong, json!(data));
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
