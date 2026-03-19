use super::{
    TransportKeyExchangeModel, TransportPeerState, TransportProvisioningMode, TransportRotationStatus,
    secure_bootstrap::{SecureBootstrapDiagnostics, SecureBootstrapSession},
};
use crate::rsa::keys::{get_fingerprint, keygen};
use chrono::Utc;
use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SecureDiagnosticCode, SecureFrame, SecureRotationMode};
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

#[test]
fn diagnostics_keep_disconnect_semantics() {
    match SecureBootstrapDiagnostics::malformed("bad frame") {
        SecureFrame::BootstrapDiagnostic(frame) => {
            assert!(matches!(frame.code, SecureDiagnosticCode::MalformedFrame));
            assert!(frame.failure.disconnect);
            assert!(frame.failure.rate_limit);
        }
        _ => panic!("expected bootstrap diagnostic frame"),
    }
}

#[test]
fn open_rejects_unapproved_state() {
    let (minion_prk, _) = keygen(2048).unwrap();
    let (_, master_pbk) = keygen(2048).unwrap();
    let (_, minion_pbk) = keygen(2048).unwrap();

    assert!(SecureBootstrapSession::open(&TransportPeerState { approved_at: None, ..state(&master_pbk, &minion_pbk) }, &minion_prk, &master_pbk).is_err());
}

#[test]
fn hello_and_ack_complete_a_bound_session() {
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
        Some(SecureRotationMode::None),
    )
    .unwrap()
    .1
    {
        SecureFrame::BootstrapAck(ack) => ack,
        _ => panic!("expected bootstrap ack"),
    };

    let established = opening.verify_ack(&state, &ack, &master_pbk).unwrap();

    assert_eq!(established.session_id(), Some("sid-1"));
    assert_eq!(established.key_id(), "kid-1");
    assert!(!established.binding().master_nonce.is_empty());
}
