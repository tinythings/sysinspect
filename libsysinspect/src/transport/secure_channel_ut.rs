use super::{
    TransportKeyExchangeModel, TransportPeerState, TransportProvisioningMode, TransportRotationStatus,
    secure_bootstrap::SecureBootstrapSession,
    secure_channel::{SECURE_MAX_PAYLOAD_SIZE, SecureChannel, SecurePeerRole},
};
use crate::rsa::keys::{get_fingerprint, keygen};
use chrono::Utc;
use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SecureFrame};
use rsa::RsaPublicKey;

fn state(master_pbk: &RsaPublicKey, minion_pbk: &RsaPublicKey) -> TransportPeerState {
    TransportPeerState {
        minion_id: "mid-1".to_string(),
        master_rsa_fingerprint: get_fingerprint(master_pbk).unwrap(),
        minion_rsa_fingerprint: get_fingerprint(minion_pbk).unwrap(),
        protocol_version: SECURE_PROTOCOL_VERSION,
        key_exchange: TransportKeyExchangeModel::EphemeralSessionKeys,
        provisioning: TransportProvisioningMode::Automatic,
        approved_at: Some(Utc::now()),
        active_key_id: Some("kid-1".to_string()),
        last_key_id: Some("kid-1".to_string()),
        last_handshake_at: None,
        rotation: TransportRotationStatus::Idle,
        updated_at: Utc::now(),
        keys: vec![],
    }
}

fn channels() -> (SecureChannel, SecureChannel) {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (opening, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let ack = match SecureBootstrapSession::accept(
        &state,
        match &hello {
            SecureFrame::BootstrapHello(hello) => hello,
            _ => panic!("expected bootstrap hello"),
        },
        &master_prk,
        &minion_pbk,
        Some("sid-1".to_string()),
        Some("kid-1".to_string()),
        None,
    )
    .unwrap()
    .1
    {
        SecureFrame::BootstrapAck(ack) => ack,
        _ => panic!("expected bootstrap ack"),
    };
    let minion = opening.verify_ack(&state, &ack, &master_pbk).unwrap();
    let master = SecureBootstrapSession::accept(
        &state,
        match &hello {
            SecureFrame::BootstrapHello(hello) => hello,
            _ => panic!("expected bootstrap hello"),
        },
        &master_prk,
        &minion_pbk,
        Some("sid-1".to_string()),
        Some("kid-1".to_string()),
        None,
    )
    .unwrap()
    .0;

    (
        SecureChannel::new(SecurePeerRole::Master, &master).unwrap(),
        SecureChannel::new(SecurePeerRole::Minion, &minion).unwrap(),
    )
}

#[test]
fn secure_channel_roundtrips_json_payloads() {
    let (mut master, mut minion) = channels();
    let frame = minion.seal(&serde_json::json!({"hello":"world"})).unwrap();
    let payload: serde_json::Value = master.open(&frame).unwrap();

    assert_eq!(payload["hello"], "world");
}

#[test]
fn secure_channel_rejects_replayed_frames() {
    let (mut master, mut minion) = channels();
    let frame = minion.seal(&serde_json::json!({"hello":"world"})).unwrap();

    let _: serde_json::Value = master.open(&frame).unwrap();
    assert!(master.open::<serde_json::Value>(&frame).is_err());
}

#[test]
fn secure_channel_rejects_out_of_sequence_frames() {
    let (mut master, mut minion) = channels();
    let _ = minion.seal(&serde_json::json!({"first":1})).unwrap();
    let frame = minion.seal(&serde_json::json!({"second":2})).unwrap();

    assert!(master.open::<serde_json::Value>(&frame).is_err());
}

#[test]
fn secure_channel_rejects_oversized_payloads() {
    let (_, mut minion) = channels();

    assert!(minion.seal_bytes(&vec![0u8; SECURE_MAX_PAYLOAD_SIZE + 1]).is_err());
}
