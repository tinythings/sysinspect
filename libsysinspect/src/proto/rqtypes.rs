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

    /// Bye
    #[serde(rename = "b")]
    Bye,

    /// Bye Ack
    #[serde(rename = "ba")]
    ByeAck,

    /// Retry connect (e.g. after the registration)
    #[serde(rename = "retry")]
    Reconnect,

    /// Unknown agent
    #[serde(rename = "undef")]
    AgentUnknown,

    /// Ping
    #[serde(rename = "pi")]
    Ping,

    /// Pong
    #[serde(rename = "po")]
    Pong,

    /// Event notice. This is called after each action/event.
    #[serde(rename = "evt")]
    Event,

    /// Model notice. This is called at the end of the model cycle
    #[serde(rename = "mvt")]
    ModelEvent,
}
