use serde::{Deserialize, Serialize};

/// This is identical to modlib::response::ModResponse but
/// can accept partially serialised data. Module does *not*
/// sends empty properties over the protocol to save the bandwidth.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionResponse {
    retcode: i32,
    warning: Option<Vec<String>>,
    message: String,
    data: Option<serde_json::Value>,
}
