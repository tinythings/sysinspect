use crate::{
    MinionMessage,
    rqtypes::{ProtoKey, RequestType},
};
use libcommon::SysinspectError;
use serde::Deserialize;

const REPLAY_KIND_EVENT: &str = "evt";
const REPLAY_KIND_MODEL_EVENT: &str = "mvt";
const REPLAY_KIND_MODEL_ACK: &str = "mack";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayIdentity {
    Event { minion_id: String, cycle_id: String, entity_id: String, session_id: String, action_id: String },
    ModelEvent { minion_id: String, cycle_id: String },
    ModelAck { minion_id: String, cycle_id: String },
}

#[derive(Debug, Deserialize)]
struct ReplayEventPayload {
    #[serde(rename = "cid")]
    cycle_id: String,
    #[serde(rename = "eid")]
    entity_id: String,
    #[serde(rename = "sid")]
    session_id: String,
    #[serde(rename = "aid")]
    action_id: String,
}

#[derive(Debug, Deserialize)]
struct ReplayModelAckPayload {
    cycle_id: String,
}

impl ReplayIdentity {
    pub fn key(&self) -> String {
        match self {
            Self::Event { minion_id, cycle_id, entity_id, session_id, action_id } => {
                format!("{REPLAY_KIND_EVENT}|{minion_id}|{cycle_id}|{entity_id}|{session_id}|{action_id}")
            }
            Self::ModelEvent { minion_id, cycle_id } => format!("{REPLAY_KIND_MODEL_EVENT}|{minion_id}|{cycle_id}"),
            Self::ModelAck { minion_id, cycle_id } => format!("{REPLAY_KIND_MODEL_ACK}|{minion_id}|{cycle_id}"),
        }
    }
}

pub fn replay_identity_from_minion_message(msg: &MinionMessage) -> Result<Option<ReplayIdentity>, SysinspectError> {
    Ok(match msg.req_type() {
        RequestType::Event => {
            let payload = serde_json::from_value::<ReplayEventPayload>(msg.payload().clone())?;
            Some(ReplayIdentity::Event {
                minion_id: msg.id().to_string(),
                cycle_id: payload.cycle_id,
                entity_id: payload.entity_id,
                session_id: payload.session_id,
                action_id: payload.action_id,
            })
        }
        RequestType::ModelEvent => {
            let cycle_id = msg.payload().get(ProtoKey::CycleId.to_string()).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            Some(ReplayIdentity::ModelEvent { minion_id: msg.id().to_string(), cycle_id })
        }
        RequestType::ModelAck => {
            let payload = serde_json::from_value::<ReplayModelAckPayload>(msg.payload().clone())?;
            Some(ReplayIdentity::ModelAck { minion_id: msg.id().to_string(), cycle_id: payload.cycle_id })
        }
        _ => None,
    })
}

pub fn replay_identity_from_minion_bytes(raw: &[u8]) -> Result<Option<ReplayIdentity>, SysinspectError> {
    replay_identity_from_minion_message(&serde_json::from_slice::<MinionMessage>(raw)?)
}
