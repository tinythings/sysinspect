use crate::{
    MasterMessage, MinionMessage, ProtoConversion,
    replay::{
        ReplayIdentity, replay_identity_for_master_command, replay_identity_for_master_command_cycle, replay_identity_from_minion_bytes,
        replay_identity_from_minion_message,
    },
    rqtypes::RequestType,
};
use serde_json::json;

#[test]
fn event_identity_is_stable_from_message_and_bytes() {
    let msg = MinionMessage::new(
        "minion-1".to_string(),
        RequestType::Event,
        json!({"eid":"entity-1","aid":"action-1","sid":"session-1","cid":"cycle-1","timestamp":"1"}),
    );

    let from_msg = replay_identity_from_minion_message(&msg).unwrap().unwrap();
    let from_bytes = replay_identity_from_minion_bytes(&msg.sendable().unwrap()).unwrap().unwrap();

    assert_eq!(
        from_msg,
        ReplayIdentity::Event {
            minion_id: "minion-1".to_string(),
            cycle_id: "cycle-1".to_string(),
            entity_id: "entity-1".to_string(),
            session_id: "session-1".to_string(),
            action_id: "action-1".to_string(),
        }
    );
    assert_eq!(from_msg, from_bytes);
    assert_eq!(from_msg.key(), "evt|minion-1|cycle-1|entity-1|session-1|action-1");
}

#[test]
fn model_event_identity_is_stable_from_message_and_bytes() {
    let msg = MinionMessage::new(
        "minion-1".to_string(),
        RequestType::ModelEvent,
        json!({"eid":"entity-1","aid":"action-1","sid":"session-1","cid":"cycle-1","timestamp":"1"}),
    );

    let from_msg = replay_identity_from_minion_message(&msg).unwrap().unwrap();
    let from_bytes = replay_identity_from_minion_bytes(&msg.sendable().unwrap()).unwrap().unwrap();

    assert_eq!(from_msg, ReplayIdentity::ModelEvent { minion_id: "minion-1".to_string(), cycle_id: "cycle-1".to_string() });
    assert_eq!(from_msg, from_bytes);
    assert_eq!(from_msg.key(), "mvt|minion-1|cycle-1");
}

#[test]
fn model_ack_identity_is_stable_from_message_and_bytes() {
    let msg = MinionMessage::new("minion-1".to_string(), RequestType::ModelAck, json!({"cycle_id":"cycle-1"}));

    let from_msg = replay_identity_from_minion_message(&msg).unwrap().unwrap();
    let from_bytes = replay_identity_from_minion_bytes(&msg.sendable().unwrap()).unwrap().unwrap();

    assert_eq!(from_msg, ReplayIdentity::ModelAck { minion_id: "minion-1".to_string(), cycle_id: "cycle-1".to_string() });
    assert_eq!(from_msg, from_bytes);
    assert_eq!(from_msg.key(), "mack|minion-1|cycle-1");
}

#[test]
fn unsupported_message_type_has_no_replay_identity() {
    let msg = MinionMessage::new("minion-1".to_string(), RequestType::Ping, json!({}));

    assert!(replay_identity_from_minion_message(&msg).unwrap().is_none());
    assert!(replay_identity_from_minion_bytes(&msg.sendable().unwrap()).unwrap().is_none());
}

#[test]
fn master_command_identity_uses_minion_and_cycle() {
    let msg = MasterMessage::new(RequestType::Command, json!({"uri":"model://demo"}));

    assert_eq!(
        replay_identity_for_master_command("minion-1", &msg),
        ReplayIdentity::MasterCommand { minion_id: "minion-1".to_string(), cycle_id: msg.cycle().clone() }
    );
    assert_eq!(
        replay_identity_for_master_command_cycle("minion-1", msg.cycle()),
        ReplayIdentity::MasterCommand { minion_id: "minion-1".to_string(), cycle_id: msg.cycle().clone() }
    );
    assert_eq!(replay_identity_for_master_command("minion-1", &msg).key(), format!("mcmd|minion-1|{}", msg.cycle()));
}
