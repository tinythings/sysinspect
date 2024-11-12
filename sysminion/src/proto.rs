pub mod msg {
    use crate::minion::{get_minion_traits, MINION_SID};
    use libsysinspect::{
        proto::{rqtypes::RequestType, MinionMessage},
        traits,
        util::dataconv,
    };
    use libsysinspect::{
        proto::{MasterMessage, ProtoConversion},
        SysinspectError,
    };

    /// Make pong message
    pub fn get_pong() -> Vec<u8> {
        let p = MinionMessage::new(
            dataconv::as_str(get_minion_traits().get(traits::SYS_ID.to_string())),
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
