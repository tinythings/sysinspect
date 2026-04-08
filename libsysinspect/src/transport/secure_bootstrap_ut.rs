use super::{
    TransportKeyExchangeModel, TransportPeerState, TransportProvisioningMode, TransportRotationStatus,
    secure_bootstrap::{SecureBootstrapDiagnostics, SecureBootstrapSession},
};
use crate::rsa::keys::{get_fingerprint, keygen, sign_data};
use base64::{Engine, engine::general_purpose::STANDARD};
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
        pending_rotation_context: None,
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

    assert!(
        SecureBootstrapSession::open(&TransportPeerState { approved_at: None, ..state(&master_pbk, &minion_pbk) }, &minion_prk, &master_pbk).is_err()
    );
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

#[test]
fn tampered_hello_key_id_is_rejected() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let mut hello = match hello {
        SecureFrame::BootstrapHello(hello) => hello,
        _ => panic!("expected bootstrap hello"),
    };
    hello.key_id = Some("kid-tampered".to_string());

    assert!(SecureBootstrapSession::accept(&state, &hello, &master_prk, &minion_pbk, None, Some("kid-1".to_string()), None).is_err());
}

#[test]
fn tampered_ack_key_id_is_rejected() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (opening, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let mut ack = match SecureBootstrapSession::accept(
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
    ack.key_id = "kid-tampered".to_string();

    assert!(opening.verify_ack(&state, &ack, &master_pbk).is_err());
}

#[test]
fn distinct_openings_derive_distinct_session_keys() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);

    let (first_opening, first_hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let first_ack = match SecureBootstrapSession::accept(
        &state,
        match &first_hello {
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
    let first = first_opening.verify_ack(&state, &first_ack, &master_pbk).unwrap();

    let (second_opening, second_hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let second_ack = match SecureBootstrapSession::accept(
        &state,
        match &second_hello {
            SecureFrame::BootstrapHello(hello) => hello,
            _ => panic!("expected bootstrap hello"),
        },
        &master_prk,
        &minion_pbk,
        Some("sid-2".to_string()),
        Some("kid-1".to_string()),
        Some(SecureRotationMode::None),
    )
    .unwrap()
    .1
    {
        SecureFrame::BootstrapAck(ack) => ack,
        _ => panic!("expected bootstrap ack"),
    };
    let second = second_opening.verify_ack(&state, &second_ack, &master_pbk).unwrap();

    assert_ne!(first.binding().connection_id, second.binding().connection_id);
    assert_ne!(first.session_key().0.to_vec(), second.session_key().0.to_vec());
}

#[test]
fn bootstrap_negotiates_down_to_supported_version() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (opening, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let mut hello = match hello {
        SecureFrame::BootstrapHello(hello) => hello,
        _ => panic!("expected bootstrap hello"),
    };
    hello.binding.protocol_version = 99;
    hello.supported_versions = vec![99, SECURE_PROTOCOL_VERSION];
    hello.binding_signature = STANDARD.encode(
        sign_data(minion_prk.clone(), &{
            let mut material = serde_json::to_vec(&hello.binding).unwrap();
            material.extend_from_slice(hello.client_ephemeral_pubkey.as_bytes());
            material.extend_from_slice(serde_json::to_string(&hello.supported_versions).unwrap().as_bytes());
            material.extend_from_slice(hello.key_id.as_deref().unwrap_or_default().as_bytes());
            material
        })
        .unwrap(),
    );

    let ack = match SecureBootstrapSession::accept(
        &state,
        &hello,
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

    assert_eq!(ack.binding.protocol_version, SECURE_PROTOCOL_VERSION);
    assert_eq!(opening.verify_ack(&state, &ack, &master_pbk).unwrap().binding().protocol_version, SECURE_PROTOCOL_VERSION);
}

#[test]
fn bootstrap_rejects_when_no_common_version_exists() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let mut hello = match hello {
        SecureFrame::BootstrapHello(hello) => hello,
        _ => panic!("expected bootstrap hello"),
    };
    hello.binding.protocol_version = 99;
    hello.supported_versions = vec![99];
    hello.binding_signature = STANDARD.encode(
        sign_data(minion_prk.clone(), &{
            let mut material = serde_json::to_vec(&hello.binding).unwrap();
            material.extend_from_slice(hello.client_ephemeral_pubkey.as_bytes());
            material.extend_from_slice(serde_json::to_string(&hello.supported_versions).unwrap().as_bytes());
            material.extend_from_slice(hello.key_id.as_deref().unwrap_or_default().as_bytes());
            material
        })
        .unwrap(),
    );

    let err = SecureBootstrapSession::accept(&state, &hello, &master_prk, &minion_pbk, None, Some("kid-1".to_string()), None).unwrap_err();
    assert!(err.to_string().contains("No common secure transport protocol version"));
}

#[test]
fn bootstrap_hello_roundtrips_through_json() {
    let (_, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let parsed = serde_json::from_slice::<SecureFrame>(&serde_json::to_vec(&hello).unwrap()).unwrap();

    assert!(matches!(parsed, SecureFrame::BootstrapHello(_)));
}

#[test]
fn accept_rejects_wrong_registered_minion_key() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let (_, wrong_minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();

    let err = SecureBootstrapSession::accept(
        &state,
        match &hello {
            SecureFrame::BootstrapHello(hello) => hello,
            _ => panic!("expected bootstrap hello"),
        },
        &master_prk,
        &wrong_minion_pbk,
        Some("sid-1".to_string()),
        Some("kid-1".to_string()),
        Some(SecureRotationMode::None),
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("fingerprint") || err.contains("signature"));
}

#[test]
fn verify_ack_rejects_wrong_master_key() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (_, wrong_master_pbk) = keygen(2048).unwrap();
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

    let err = opening.verify_ack(&state, &ack, &wrong_master_pbk).unwrap_err().to_string();
    assert!(err.contains("fingerprint") || err.contains("signature"));
}

#[test]
fn verify_ack_rejects_invalid_master_ephemeral_key() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (opening, hello) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let mut ack = match SecureBootstrapSession::accept(
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
    ack.master_ephemeral_pubkey = STANDARD.encode([0u8; 8]);

    let err = opening.verify_ack(&state, &ack, &master_pbk).unwrap_err().to_string();
    assert!(err.contains("invalid size"));
}
