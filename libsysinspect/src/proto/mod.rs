pub mod errcodes;
pub mod rqtypes;

use crate::SysinspectError;
use errcodes::ProtoErrorCode;
use rqtypes::RequestType;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// Master message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterMessage {
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
        MasterMessage { target: Default::default(), request: rtype, data, retcode: ProtoErrorCode::Undef as usize }
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
    pub fn get_target(&self) -> &MinionTarget {
        &self.target
    }
}

/// Minion message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinionMessage {
    id: String,
    sid: String, // Temporary session Id

    #[serde(rename = "r")]
    request: RequestType,

    #[serde(rename = "d")]
    data: String,

    #[serde(rename = "c")]
    retcode: usize,
}

impl MinionMessage {
    /// Message constructor
    pub fn new(id: String, rtype: RequestType, data: String) -> MinionMessage {
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

    /// Get payload
    pub fn payload(&self) -> &str {
        &self.data
    }
}

/// Minion target
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MinionTarget {
    /// List of minion Ids
    id: String,

    /// Session Id
    sid: String, // XXX: Should be gone

    /// Scheme to call (model:// or state://)
    #[serde(rename = "s")]
    scheme: String,

    /// Traits query that needs to be parsed at Minion
    #[serde(rename = "qt")]
    traits_query: String,

    #[serde(rename = "h")]
    hostnames: Vec<String>,
}

impl MinionTarget {
    pub fn new(mid: &str, sid: &str) -> MinionTarget {
        MinionTarget { id: mid.to_string(), sid: sid.to_string(), ..Default::default() }
    }

    /// Add hostnames
    pub fn add_hostname(&mut self, hostname: &str) {
        self.hostnames.push(hostname.to_string());
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn sid(&self) -> &String {
        &self.sid
    }

    pub fn hostnames(&self) -> &Vec<String> {
        &self.hostnames
    }

    /// Set scheme
    pub fn set_scheme(&mut self, scheme: &str) {
        self.scheme = scheme.to_string();
    }

    /// Set traits query
    pub fn set_traits_query(&mut self, traits: &str) {
        self.traits_query = traits.to_string();
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
