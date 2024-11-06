use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RequestType {
    /// Minion registration request or context.
    #[serde(rename = "add")]
    Add,

    /// Minion un-registration request.
    #[serde(rename = "rm")]
    Remove,

    /// Regular response to any Master command
    #[serde(rename = "rsp")]
    Response,

    /// Regular command to any Minion
    #[serde(rename = "cmd")]
    Command,

    /// Request to return all minion traits
    #[serde(rename = "tr")]
    Traits,

    /// Hello/ehlo
    #[serde(rename = "ehlo")]
    Ehlo,

    /// Unknown agent
    #[serde(rename = "undef")]
    AgentUnknown,
}
