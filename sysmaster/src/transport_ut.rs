use super::{PeerConnection, PeerTransport};
use chrono::Utc;
use libsysinspect::{
    rsa::keys::{get_fingerprint, keygen},
    transport::{
        TransportKeyExchangeModel, TransportPeerState, TransportProvisioningMode, TransportRotationStatus,
        secure_bootstrap::SecureBootstrapSession,
        secure_channel::{SecureChannel, SecurePeerRole},
    },
};
use libsysproto::{
    MasterMessage, ProtoConversion,
    rqtypes::RequestType,
    secure::{SECURE_PROTOCOL_VERSION, SecureDiagnosticCode, SecureFrame},
};
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

fn channels() -> (SecureChannel, SecureChannel) {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (opening, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let accepted = SecureBootstrapSession::accept(
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
    .unwrap();
    let ack = match accepted.1 {
        SecureFrame::BootstrapAck(ack) => ack,
        _ => panic!("expected bootstrap ack"),
    };
    let minion = opening.verify_ack(&state, &ack, &master_pbk).unwrap();
    let master = accepted.0;

    (SecureChannel::new(SecurePeerRole::Master, &master).unwrap(), SecureChannel::new(SecurePeerRole::Minion, &minion).unwrap())
}

#[test]
fn encode_message_stays_plaintext_without_session() {
    let mut transport = PeerTransport::new();
    let msg = MasterMessage::new(RequestType::Ping, serde_json::json!("general"));

    let encoded = transport.encode_message("127.0.0.1:4200", &msg).unwrap();

    assert_eq!(encoded, msg.sendable().unwrap());
}

#[test]
fn encode_message_seals_payload_with_active_session() {
    let mut transport = PeerTransport::new();
    let (master_channel, mut minion_channel) = channels();
    transport.peers.insert("127.0.0.1:4200".to_string(), PeerConnection { minion_id: "mid-1".to_string(), channel: master_channel });
    let msg = MasterMessage::new(RequestType::Ping, serde_json::json!("general"));

    let encoded = transport.encode_message("127.0.0.1:4200", &msg).unwrap();
    let opened = minion_channel.open_bytes(&encoded).unwrap();

    assert_eq!(opened, msg.sendable().unwrap());
}

#[test]
fn decode_frame_drops_invalid_secure_session_state() {
    let mut transport = PeerTransport::new();
    let (master_channel, mut minion_channel) = channels();
    transport.peers.insert("127.0.0.1:4200".to_string(), PeerConnection { minion_id: "mid-1".to_string(), channel: master_channel });
    let mut frame = minion_channel.seal(&serde_json::json!({"hello":"world"})).unwrap();
    frame.pop();

    let root = tempfile::tempdir().unwrap();
    let err = transport
        .decode_frame(
            "127.0.0.1:4200",
            "127.0.0.1:4200",
            &frame,
            &libsysinspect::cfg::mmconf::MasterConfig::default(),
            &mut crate::registry::mkb::MinionsKeyRegistry::new(root.path().to_path_buf()).unwrap(),
        )
        .unwrap_err()
        .to_string();

    assert!(err.contains("decode secure frame") || err.contains("decode secure payload") || err.contains("Expected encrypted secure data frame"));
    assert!(!transport.peers.contains_key("127.0.0.1:4200"));
}

#[test]
fn remove_peer_clears_secure_and_plaintext_tracking() {
    let mut transport = PeerTransport::new();
    let (master_channel, _) = channels();
    transport.allow_plaintext("127.0.0.1:4200");
    transport.peers.insert("127.0.0.1:4200".to_string(), PeerConnection { minion_id: "mid-1".to_string(), channel: master_channel });

    transport.remove_peer("127.0.0.1:4200");

    assert!(!transport.peers.contains_key("127.0.0.1:4200"));
    assert!(!transport.plaintext_peers.contains("127.0.0.1:4200"));
}

#[test]
fn peer_addr_finds_existing_minion_session() {
    let mut transport = PeerTransport::new();
    let (master_channel, _) = channels();
    transport.peers.insert("127.0.0.1:4200".to_string(), PeerConnection { minion_id: "mid-1".to_string(), channel: master_channel });

    assert_eq!(transport.peer_addr("mid-1", "127.0.0.1:4201").as_deref(), Some("127.0.0.1:4200"));
    assert!(transport.peer_addr("mid-1", "127.0.0.1:4200").is_none());
}

#[test]
fn plaintext_diag_rejects_non_registration_traffic() {
    let diag = PeerTransport::plaintext_diag(br#"{"id":"mid-1","r":"ehlo","d":{},"c":0,"sid":"sid-1"}"#).unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&diag).unwrap(),
        SecureFrame::BootstrapDiagnostic(frame)
            if matches!(frame.code, SecureDiagnosticCode::BootstrapRejected)
                && frame.message.contains("secure bootstrap is required")
    ));
}

#[test]
fn bootstrap_diag_ignores_non_secure_plaintext() {
    let mut failures = std::collections::HashMap::new();

    assert!(PeerTransport::bootstrap_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"hello":"world"}"#).is_none());
}
