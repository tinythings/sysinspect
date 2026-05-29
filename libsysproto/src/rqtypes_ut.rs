use crate::rqtypes::{OutboundMessageClass, RequestType};

// ── durable data: execution results that must survive delivery failure ──

#[test]
fn event_is_durable_data() {
    assert_eq!(RequestType::Event.message_class(), OutboundMessageClass::DurableData);
}

#[test]
fn model_event_is_durable_data() {
    assert_eq!(RequestType::ModelEvent.message_class(), OutboundMessageClass::DurableData);
}

#[test]
fn model_ack_is_durable_data() {
    assert_eq!(RequestType::ModelAck.message_class(), OutboundMessageClass::DurableData);
}

// ── session / control: connection-maintenance messages ──

#[test]
fn ehlo_is_session_control() {
    assert_eq!(RequestType::Ehlo.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn traits_is_session_control() {
    assert_eq!(RequestType::Traits.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn ping_is_session_control() {
    assert_eq!(RequestType::Ping.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn pong_is_session_control() {
    assert_eq!(RequestType::Pong.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn sensors_sync_request_is_session_control() {
    assert_eq!(RequestType::SensorsSyncRequest.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn sensors_sync_response_is_session_control() {
    assert_eq!(RequestType::SensorsSyncResponse.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn bye_is_session_control() {
    assert_eq!(RequestType::Bye.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn bye_ack_is_session_control() {
    assert_eq!(RequestType::ByeAck.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn add_is_session_control() {
    assert_eq!(RequestType::Add.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn remove_is_session_control() {
    assert_eq!(RequestType::Remove.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn command_is_session_control() {
    assert_eq!(RequestType::Command.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn response_is_session_control() {
    assert_eq!(RequestType::Response.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn reconnect_is_session_control() {
    assert_eq!(RequestType::Reconnect.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn agent_unknown_is_session_control() {
    assert_eq!(RequestType::AgentUnknown.message_class(), OutboundMessageClass::SessionControl);
}

#[test]
fn cycle_ack_is_session_control() {
    assert_eq!(RequestType::CycleAck.message_class(), OutboundMessageClass::SessionControl);
}

// ── design contract: exactly these three types are durable, nothing else ──

#[test]
fn only_the_three_journaled_types_are_durable_data() {
    for rt in &[
        RequestType::Add,
        RequestType::Remove,
        RequestType::Response,
        RequestType::Command,
        RequestType::Traits,
        RequestType::Ehlo,
        RequestType::Bye,
        RequestType::ByeAck,
        RequestType::Reconnect,
        RequestType::AgentUnknown,
        RequestType::Ping,
        RequestType::Pong,
        RequestType::SensorsSyncRequest,
        RequestType::SensorsSyncResponse,
        RequestType::CycleAck,
    ] {
        assert_eq!(
            rt.message_class(),
            OutboundMessageClass::SessionControl,
            "{rt:?} must remain SessionControl per the offline-independence design contract"
        );
    }
}

#[test]
fn durable_data_variants_match_the_documented_set() {
    for rt in &[RequestType::Event, RequestType::ModelEvent, RequestType::ModelAck] {
        assert_eq!(rt.message_class(), OutboundMessageClass::DurableData, "{rt:?} must be DurableData per spec");
    }
}
