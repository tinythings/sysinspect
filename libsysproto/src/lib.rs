pub mod errcodes;
pub mod payload;
pub mod query;
pub mod rqtypes;

use std::collections::HashSet;

use errcodes::ProtoErrorCode;
use libcommon::SysinspectError;
use rqtypes::RequestType;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

/// Master message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterMessage {
    /// Message Id which is used as cycle ID
    #[serde(rename = "cl")]
    cycle: String,

    #[serde(rename = "t")]
    target: MinionTarget,

    #[serde(rename = "r")]
    request: RequestType,

    #[serde(rename = "d")]
    data: Value,

    #[serde(rename = "c")]
    retcode: usize,
}

impl MasterMessage {
    /// Master message constructor
    pub fn new(rtype: RequestType, data: Value) -> MasterMessage {
        MasterMessage { target: Default::default(), request: rtype, data, retcode: ProtoErrorCode::Undef as usize, cycle: Uuid::new_v4().to_string() }
    }

    /// Creates a command message with a new cycle ID. This is used by the pipeline handler to create a master
    /// message with the URI specified in the event configuration.
    pub fn command() -> MasterMessage {
        MasterMessage::new(RequestType::Command, json!({"files": {}, "sid": "", "models_root": "", "sensors_root":"", "uri": ""}))
    }

    /// Add a target.
    pub fn set_target(&mut self, t: MinionTarget) {
        self.target = t;
    }

    /// Set return code
    pub fn set_retcode(&mut self, retcode: ProtoErrorCode) {
        self.retcode = retcode as usize;
    }

    /// Get return code
    pub fn get_retcode(&self) -> ProtoErrorCode {
        match &self.retcode {
            0 => ProtoErrorCode::Undef,
            1 => ProtoErrorCode::Success,
            2 => ProtoErrorCode::GeneralFailure,
            3 => ProtoErrorCode::NotRegistered,
            4 => ProtoErrorCode::AlreadyRegistered,
            5 => ProtoErrorCode::AlreadyConnected,
            _ => ProtoErrorCode::Unknown,
        }
    }

    /// Request type
    pub fn req_type(&self) -> &RequestType {
        &self.request
    }

    /// Get payload
    pub fn payload(&self) -> &Value {
        &self.data
    }

    /// Get targeting means
    pub fn target(&self) -> &MinionTarget {
        &self.target
    }

    /// Get cycle ID (message ID)
    pub fn cycle(&self) -> &String {
        &self.cycle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinionMessageData {
    eid: String,
    aid: String,
    sid: String,
    cid: String,
    timestamp: String,
    response: MinionMessageResponse,
    constraints: Value,
    telemetry: Value,
}

impl MinionMessageData {
    /// Get cycle ID
    pub fn cid(&self) -> &String {
        &self.cid
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinionMessageResponse {
    retcode: usize,
    warning: Option<String>,
    message: String,
    data: Value,
}

/// Minion message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinionMessage {
    id: String,
    sid: String, // Temporary session Id

    #[serde(rename = "r")]
    request: RequestType,

    #[serde(rename = "d")]
    data: Value,

    #[serde(rename = "c")]
    retcode: usize,
}

impl MinionMessage {
    /// Message constructor
    pub fn new(id: String, rtype: RequestType, data: Value) -> MinionMessage {
        MinionMessage { id, request: rtype, data, retcode: ProtoErrorCode::Undef as usize, sid: "".to_string() }
    }

    /// Set return code
    pub fn set_retcode(&mut self, retcode: ProtoErrorCode) {
        self.retcode = retcode as usize;
    }

    /// Get return code
    pub fn get_retcode(&self) -> ProtoErrorCode {
        match &self.retcode {
            0 => ProtoErrorCode::Undef,
            1 => ProtoErrorCode::Success,
            2 => ProtoErrorCode::GeneralFailure,
            3 => ProtoErrorCode::NotRegistered,
            4 => ProtoErrorCode::AlreadyRegistered,
            5 => ProtoErrorCode::AlreadyConnected,
            _ => ProtoErrorCode::Unknown,
        }
    }

    /// Set Session Id
    pub fn set_sid(&mut self, sid: String) {
        self.sid = sid
    }

    /// Request type
    pub fn req_type(&self) -> &RequestType {
        &self.request
    }

    /// Get minion Id
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get session Id
    pub fn sid(&self) -> &str {
        &self.sid
    }

    /// Get payload
    pub fn payload(&self) -> &Value {
        &self.data
    }

    /// Get data as MinionMessageData
    pub fn get_data(&self) -> MinionMessageData {
        match serde_json::from_value::<MinionMessageData>(self.data.clone()) {
            Ok(data) => data,
            Err(_) => MinionMessageData {
                eid: "".to_string(),
                aid: "".to_string(),
                sid: "".to_string(),
                cid: "".to_string(),
                timestamp: "".to_string(),
                response: MinionMessageResponse {
                    retcode: ProtoErrorCode::GeneralFailure as usize,
                    warning: None,
                    message: "Unable to parse MinionMessageData structure".to_string(),
                    data: Value::Null,
                },
                constraints: Value::Null,
                telemetry: Value::Null,
            },
        }
    }
}

/// Minion target
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MinionTarget {
    /// List of minion Ids
    id: String,

    /// Session Id
    sid: String, // XXX: Should be gone

    /// Which scheme must be called (model://)
    #[serde(rename = "s")]
    scheme: String,

    /// Traits query that needs to be parsed at Minion
    #[serde(rename = "qt")]
    traits_query: String,

    #[serde(rename = "h")]
    hostnames: HashSet<String>,

    #[serde(rename = "cq")]
    context_query: String,
}

impl MinionTarget {
    pub fn new(mid: &str, sid: &str) -> MinionTarget {
        MinionTarget { id: mid.to_string(), sid: sid.to_string(), ..Default::default() }
    }

    /// Add hostnames
    pub fn add_hostname(&mut self, hostname: &str) {
        self.hostnames.insert(hostname.to_string());
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn sid(&self) -> &String {
        &self.sid
    }

    pub fn hostnames(&self) -> Vec<String> {
        self.hostnames.iter().map(|s| s.to_string()).collect()
    }

    /// Get scheme
    pub fn scheme(&self) -> &String {
        &self.scheme
    }

    /// Get context query
    pub fn context(&self) -> &String {
        &self.context_query
    }

    /// Set scheme
    pub fn set_scheme(&mut self, scheme: &str) {
        self.scheme = scheme.to_string();
    }

    /// Set traits query
    pub fn set_traits_query(&mut self, traits: &str) {
        self.traits_query = traits.to_string();
    }

    /// Get context query
    pub fn set_context_query(&mut self, context: &str) {
        self.context_query = context.to_string();
    }

    /// Traits query itself.
    pub fn traits_query(&self) -> &String {
        &self.traits_query
    }
}

pub trait ProtoConversion: Serialize + DeserializeOwned {
    fn serialise(&self) -> Result<String, SysinspectError>;
    fn sendable(&self) -> Result<Vec<u8>, SysinspectError>;
}

impl<T> ProtoConversion for T
where
    T: Serialize + DeserializeOwned,
{
    /// Serialise self
    fn serialise(&self) -> Result<String, SysinspectError> {
        match serde_json::to_string(self) {
            Ok(out) => Ok(out),
            Err(err) => Err(SysinspectError::MinionGeneralError(format!("{err}"))),
        }
    }

    /// Serialise self to bytes
    fn sendable(&self) -> Result<Vec<u8>, SysinspectError> {
        Ok(self.serialise()?.as_bytes().to_vec())
    }
}
