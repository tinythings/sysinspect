use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub fn retcode(&self) -> i32 {
        self.retcode
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

    pub fn data(&self) -> Option<Value> {
        self.data.to_owned()
    }
}

#[derive(Debug, Default, Clone)]
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
        if self.sid.eq("") {
            "$"
        } else {
            &self.sid
        }
    }

    /// Match Eid.
    /// Event Id parts can be also substituted to `$` (any).
    ///
    /// Error codes:
    ///   - `$`       - any
    ///   - `0..255`  - specific code
    ///   - `E`       - error only (non-0)
    ///
    pub fn match_eid(&self, eid: &str) -> bool {
        let p_eid = eid.split('/').map(|s| s.trim()).collect::<Vec<&str>>();

        // Have fun reading this :-P
        p_eid.len() == 4
            && (self.aid().eq(p_eid[0]) || p_eid[0] == "$")
            && (self.eid().eq(p_eid[1]) || p_eid[1] == "$")
            && (self.sid().eq(p_eid[2]) || p_eid[2] == "$")
            && ((p_eid[3] == "$")
                || (p_eid[3].eq("E") && self.response.retcode() > 0)
                || p_eid[3].eq(&self.response.retcode().to_string()))
    }
}
