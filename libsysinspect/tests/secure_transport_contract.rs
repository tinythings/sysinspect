use chrono::{Duration as ChronoDuration, Utc};
use libsysinspect::{
    rsa::{
        keys::{get_fingerprint, keygen},
        rotation::{RotationActor, RsaTransportRotator},
    },
    transport::{
        TransportKeyExchangeModel, TransportKeyStatus, TransportPeerState, TransportProvisioningMode, TransportRotationStatus, TransportStore,
        secure_bootstrap::SecureBootstrapSession,
        secure_channel::{SecureChannel, SecurePeerRole},
    },
};
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
        pending_rotation_context: None,
        updated_at: Utc::now(),
        keys: vec![],
    }
}

fn establish_channels(state: &TransportPeerState) -> (SecureChannel, SecureChannel) {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let rebound = TransportPeerState {
        master_rsa_fingerprint: get_fingerprint(&master_pbk).unwrap(),
        minion_rsa_fingerprint: get_fingerprint(&minion_pbk).unwrap(),
        ..state.clone()
    };
    let (opening, hello) = SecureBootstrapSession::open(&rebound, &minion_prk, &master_pbk).unwrap();
    let accepted = SecureBootstrapSession::accept(
        &rebound,
        match &hello {
            SecureFrame::BootstrapHello(hello) => hello,
            _ => panic!("expected bootstrap hello"),
        },
        &master_prk,
        &minion_pbk,
        Some("sid-1".to_string()),
        rebound.active_key_id.clone(),
        None,
    )
    .unwrap();
    let ack = match accepted.1 {
        SecureFrame::BootstrapAck(ack) => ack,
        _ => panic!("expected bootstrap ack"),
    };
    let minion = opening.verify_ack(&rebound, &ack, &master_pbk).unwrap();
    let master = accepted.0;

    (SecureChannel::new(SecurePeerRole::Master, &master).unwrap(), SecureChannel::new(SecurePeerRole::Minion, &minion).unwrap())
}

#[test]
fn automatic_transport_state_establishes_secure_session_roundtrip() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (_, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (mut master, mut minion) = establish_channels(&state);

    let frame = minion.seal(&serde_json::json!({"kind":"ping","value":1})).unwrap();
    let payload: serde_json::Value = master.open(&frame).unwrap();

    assert_eq!(payload["kind"], "ping");
    assert_eq!(payload["value"], 1);
    assert!(!master.session_id().is_empty());
    assert!(!minion.session_id().is_empty());
    drop(master_prk);
}

#[test]
fn public_transport_api_rejects_replayed_frames() {
    let (_, master_pbk) = keygen(2048).unwrap();
    let (_, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (mut master, mut minion) = establish_channels(&state);

    let frame = minion.seal(&serde_json::json!({"kind":"ping"})).unwrap();
    let _: serde_json::Value = master.open(&frame).unwrap();

    assert!(master.open::<serde_json::Value>(&frame).is_err());
}

#[test]
fn rotated_transport_state_reconnects_with_new_key_id() {
    let root = tempfile::tempdir().unwrap();
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (_, minion_pbk) = keygen(2048).unwrap();
    let store = TransportStore::new(root.path().join("transport/master/state.json")).unwrap();
    let mut state = state(&master_pbk, &minion_pbk);
    store.save(&state).unwrap();

    let mut rotator = RsaTransportRotator::new(
        RotationActor::Master,
        store,
        &state.minion_id,
        &state.master_rsa_fingerprint,
        &state.minion_rsa_fingerprint,
        SECURE_PROTOCOL_VERSION,
    )
    .unwrap();
    let plan = rotator.plan("manual");
    let signed = rotator.sign_plan(&plan, &master_prk).unwrap();
    let rollback = rotator.execute_signed_intent_with_overlap(&signed, &RsaPublicKey::from(&master_prk), ChronoDuration::seconds(60)).unwrap();
    state = rotator.state().clone();

    assert_eq!(state.rotation, TransportRotationStatus::Idle);
    assert_eq!(state.active_key_id.as_deref(), Some(signed.intent().next_key_id()));
    assert!(state.keys.iter().any(|record| record.status == TransportKeyStatus::Retiring));

    let (mut master, mut minion) = establish_channels(&state);
    let frame = minion.seal(&serde_json::json!({"kind":"after-rotation"})).unwrap();
    let payload: serde_json::Value = master.open(&frame).unwrap();
    assert_eq!(payload["kind"], "after-rotation");

    rotator.rollback(&rollback).unwrap();
}

#[test]
fn bootstrap_wire_shape_roundtrips_through_json() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let wire = serde_json::to_vec(&hello).unwrap();
    let parsed = serde_json::from_slice::<SecureFrame>(&wire).unwrap();

    let ack = match SecureBootstrapSession::accept(
        &state,
        match &parsed {
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

    assert_eq!(ack.key_id, "kid-1");
}
