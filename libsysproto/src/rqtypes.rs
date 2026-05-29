use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtoValue {
    #[serde(rename = "pt:g")]
    PingTypeGeneral,

    #[serde(rename = "pt:d")]
    PingTypeDiscovery,
}

#[derive(Serialize, Deserialize, Debug, Clone, Display)]
pub enum ProtoKey {
    /// Session Id
    #[strum(serialize = "sid")]
    #[serde(rename = "sid")]
    SessionId,

    /// Cycle Id
    #[strum(serialize = "cid")]
    #[serde(rename = "cid")]
    CycleId,

    /// Action Id
    #[strum(serialize = "aid")]
    #[serde(rename = "aid")]
    ActionId,

    /// Entity Id
    #[strum(serialize = "eid")]
    #[serde(rename = "eid")]
    EntityId,

    /// Protocol Type
    #[strum(serialize = "pt")]
    #[serde(rename = "pt")]
    ProtoType,

    /// Payload
    #[strum(serialize = "pl")]
    #[serde(rename = "pl")]
    Payload,

    /// Constraints
    #[strum(serialize = "constraints")]
    #[serde(rename = "constraints")]
    Constraints,

    /// Response
    #[strum(serialize = "response")]
    #[serde(rename = "response")]
    Response,

    /// Timestamp
    #[strum(serialize = "timestamp")]
    #[serde(rename = "timestamp")]
    Timestamp,

    /// Telemetry
    #[strum(serialize = "telemetry")]
    #[serde(rename = "telemetry")]
    Telemetry,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

    #[serde(rename = "ssr")]
    SensorsSyncRequest,

    #[serde(rename = "ssp")]
    SensorsSyncResponse,

    /// Minion→Master: model cycle completed.
    #[serde(rename = "mack")]
    ModelAck,

    /// Master→Minion: model cycle acknowledged.
    #[serde(rename = "cack")]
    CycleAck,
}

/// Classifies an outbound minion message so the transport layer can decide
/// whether a delivery failure should tear down the execution runtime or
/// merely degrade transport health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundMessageClass {
    /// Execution results that are journaled before send and can survive
    /// temporary delivery failure without stopping local work.
    DurableData,

    /// Session or control messages whose failure may indicate a broken
    /// connection that warrants reconnect/handshake recovery.
    SessionControl,
}

impl RequestType {
    pub fn message_class(&self) -> OutboundMessageClass {
        match self {
            RequestType::Event | RequestType::ModelEvent | RequestType::ModelAck => OutboundMessageClass::DurableData,
            _ => OutboundMessageClass::SessionControl,
        }
    }
}
