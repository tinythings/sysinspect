// XXX: Refactor: move all message types-related code here!

/*
Payload types and their deserialisation.
*/

use serde::{Deserialize, Serialize};
use serde_json::{from_value, Value};
use std::collections::HashMap;

/// Payload types
pub enum PayloadType {
    ModelOrStatement(ModStatePayload),
    Undef(Value),
}

impl TryFrom<Value> for PayloadType {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Ok(v) = from_value::<ModStatePayload>(value.clone()) {
            return Ok(PayloadType::ModelOrStatement(v));
        }
        Ok(PayloadType::Undef(value))
    }
}

/// Message is sent to the Minion
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModStatePayload {
    // Each file has a SHA1 checksum to prevent huge bogus traffic
    files: HashMap<String, String>,

    // Root where models starts. It corresponds to "fileserver.models.root" conf of Master.
    // It will be substracted from each file path when saving
    models_root: String,

    // session Id
    sid: String,

    // sysinspect URI
    uri: String,
}

impl ModStatePayload {
    pub fn new(sid: String) -> Self {
        ModStatePayload { sid, ..Default::default() }
    }

    /// Set URI
    pub fn set_uri(mut self, uri: String) -> Self {
        self.uri = uri;
        self
    }

    /// Add files
    pub fn add_files(mut self, files: HashMap<String, String>) -> Self {
        self.files.extend(files);
        self
    }

    /// Set models root
    pub fn set_models_root(mut self, mr: &str) -> Self {
        self.models_root = mr.to_string();
        self
    }

    /// Get list of files to download
    pub fn files(&self) -> &HashMap<String, String> {
        &self.files
    }

    /// Get SID
    pub fn sid(&self) -> &str {
        &self.sid
    }

    /// Get URI
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Get root of models
    pub fn models_root(&self) -> &str {
        &self.models_root
    }
}
