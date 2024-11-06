pub mod errcodes;
pub mod rqtypes;

use crate::SysinspectError;
use errcodes::ProtoErrorCode;
use rqtypes::RequestType;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Master message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterMessage {
    #[serde(rename = "t")]
    target: Vec<MinionTarget>,

    #[serde(rename = "r")]
    request: RequestType,

    #[serde(rename = "d")]
    data: String,

    #[serde(rename = "c")]
    retcode: usize,
}

impl MasterMessage {
    /// Master message constructor
    pub fn new(rtype: RequestType, data: String) -> MasterMessage {
        MasterMessage { target: vec![], request: rtype, data, retcode: ProtoErrorCode::Undef as usize }
    }

    /// Add a target.
    pub fn add_target(&mut self, t: MinionTarget) {
        self.target.push(t);
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
            _ => ProtoErrorCode::Unknown,
        }
    }

    /// Request type
    pub fn req_type(&self) -> &RequestType {
        &self.request
    }

    /// Get payload
    pub fn payload(&self) -> &str {
        &self.data
    }
}

/// Minion message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinionMessage {
    id: String,

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
        MinionMessage { id, request: rtype, data, retcode: ProtoErrorCode::Undef as usize }
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
            _ => ProtoErrorCode::Unknown,
        }
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
    id: Vec<String>,

    /// List of a collection of traits
    #[serde(rename = "t")]
    traits: HashMap<String, Value>,

    #[serde(rename = "h")]
    hostnames: Vec<String>,
}

impl MinionTarget {
    pub fn new() -> MinionTarget {
        MinionTarget::default()
    }

    /// Add target id
    pub fn add_minion_id(&mut self, id: String) {
        self.id.push(id);
    }

    /// Add targeting trait
    pub fn add_trait(&mut self, tid: String, v: Value) {
        self.traits.insert(tid, v);
    }

    /// Add hostnames
    pub fn add_hostname(&mut self, hostname: String) {
        self.hostnames.push(hostname);
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
