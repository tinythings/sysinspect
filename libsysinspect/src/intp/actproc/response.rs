use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionModResponse {
    // Return code
    retcode: i32,

    // Warnings collection
    warning: Option<Vec<String>>,

    // General message
    message: String,

    // Arbitrary payload data
    data: Option<serde_json::Value>,
}

impl ActionModResponse {
    /// Get a return code
    pub fn retcode(&self) -> &i32 {
        &self.retcode
    }

    /// Return collected warnings
    pub fn warnings(&self) -> Vec<String> {
        if let Some(w) = &self.warning {
            return w.to_owned();
        }
        vec![]
    }

    /// Get a general return message
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Default)]
/// This is identical to modlib::response::ModResponse but
/// can accept partially serialised data. Module does *not*
/// sends empty properties over the protocol to save the bandwidth.
pub struct ActionResponse {
    // Entity Id
    eid: String,

    // Action Id
    aid: String,

    // State Id
    sid: String,

    // Module response
    pub response: ActionModResponse,
}

impl ActionResponse {
    pub(crate) fn new(eid: String, aid: String, sid: String, response: ActionModResponse) -> Self {
        Self { eid, aid, sid, response }
    }

    /// Return an Entity Id to which this action was bound to
    pub fn eid(&self) -> &str {
        &self.eid
    }

    /// Return action Id
    pub fn aid(&self) -> &str {
        &self.aid
    }

    /// Return state Id of the action
    pub fn sid(&self) -> &str {
        &self.sid
    }
}
