use crate::master::SysMaster;
use chrono::Utc;
use libsysinspect::{
    rsa::keys::{get_fingerprint, keygen},
    transport::{
        TransportKeyExchangeModel, TransportPeerState, TransportProvisioningMode, TransportRotationStatus, secure_bootstrap::SecureBootstrapSession,
    },
};
use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SECURE_SUPPORTED_PROTOCOL_VERSIONS, SecureBootstrapHello, SecureFrame, SecureSessionBinding};
use rsa::RsaPublicKey;
use std::{collections::HashMap, time::Instant};

fn fresh_timestamp() -> i64 {
    Utc::now().timestamp()
}

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
fn unsupported_peer_bounces_secure_bootstrap_hello() {
    let bounced = SysMaster::secure_peer_diag_with_state(
        &mut HashMap::<String, (Instant, u32)>::new(),
        "127.0.0.1:4200",
        &serde_json::to_vec(&SecureFrame::BootstrapHello(SecureBootstrapHello {
            binding: SecureSessionBinding::bootstrap_opening(
                "mid-1".to_string(),
                "minion-fp".to_string(),
                "master-fp".to_string(),
                "conn-1".to_string(),
                "nonce-1".to_string(),
                fresh_timestamp(),
            ),
            supported_versions: SECURE_SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
            client_ephemeral_pubkey: "pubkey".to_string(),
            binding_signature: "sig".to_string(),
            key_id: Some("kid-1".to_string()),
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(matches!(serde_json::from_slice::<SecureFrame>(&bounced).unwrap(), SecureFrame::BootstrapDiagnostic(_)));
}

#[test]
fn unsupported_peer_ignores_legacy_plaintext_messages() {
    let mut failures = HashMap::<String, (Instant, u32)>::new();

    assert!(SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"id":"mid-1","r":"ehlo","d":{},"c":1,"sid":""}"#).is_none());
    assert!(SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#).is_some());
    assert!(SECURE_PROTOCOL_VERSION > 0);
}

#[test]
fn plaintext_registration_request_remains_allowed() {
    assert!(SysMaster::plaintext_peer_diag(br#"{"id":"mid-1","r":"add","d":"pem","c":0,"sid":""}"#).is_none());
}

#[test]
fn plaintext_ehlo_is_rejected_when_secure_transport_is_enabled() {
    let bounced = SysMaster::plaintext_peer_diag(br#"{"id":"mid-1","r":"ehlo","d":{},"c":1,"sid":"sid-1"}"#).unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&bounced).unwrap(),
        SecureFrame::BootstrapDiagnostic(frame)
            if frame.message.contains("secure bootstrap is required")
    ));
}

#[test]
fn broadcasts_are_blocked_for_prebootstrap_peers() {
    assert!(!SysMaster::peer_can_receive_broadcast_state(false, false));
    assert!(SysMaster::peer_can_receive_broadcast_state(false, true));
    assert!(SysMaster::peer_can_receive_broadcast_state(true, false));
}

#[test]
fn malformed_secure_bootstrap_attempts_are_rate_limited() {
    let mut failures = HashMap::<String, (Instant, u32)>::new();

    let _ = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#);
    let _ = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#);
    let bounced = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#).unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&bounced).unwrap(),
        SecureFrame::BootstrapDiagnostic(frame) if frame.message.contains("Repeated malformed")
    ));
}

#[test]
fn malformed_secure_bootstrap_rate_limit_is_keyed_by_ip_not_port() {
    let mut failures = HashMap::<String, (Instant, u32)>::new();

    let _ = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#);
    let _ = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4201", br#"{"kind":"bootstrap_hello","broken":}"#);
    let bounced = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4202", br#"{"kind":"bootstrap_hello","broken":}"#).unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&bounced).unwrap(),
        SecureFrame::BootstrapDiagnostic(frame) if frame.message.contains("Repeated malformed")
    ));
}

#[test]
fn replay_cache_rejects_duplicate_bootstrap_openings() {
    let mut cache = HashMap::<String, Instant>::new();
    let binding = SecureSessionBinding::bootstrap_opening(
        "mid-1".to_string(),
        "minion-fp".to_string(),
        "master-fp".to_string(),
        "conn-1".to_string(),
        "nonce-1".to_string(),
        fresh_timestamp(),
    );

    assert!(SysMaster::bootstrap_precheck_with_state_for_test(&mut cache, &binding, Instant::now(),).is_none());
    SysMaster::record_bootstrap_replay_with_state_for_test(&mut cache, &binding, Instant::now());
    assert!(SysMaster::bootstrap_precheck_with_state_for_test(&mut cache, &binding, Instant::now(),).is_some());
}

#[test]
fn replay_cache_rejects_stale_bootstrap_openings_before_auth() {
    let mut cache = HashMap::<String, Instant>::new();
    let binding = SecureSessionBinding::bootstrap_opening(
        "mid-1".to_string(),
        "minion-fp".to_string(),
        "master-fp".to_string(),
        "conn-1".to_string(),
        "nonce-1".to_string(),
        fresh_timestamp() - 301,
    );

    let rejection = SysMaster::bootstrap_precheck_with_state_for_test(&mut cache, &binding, Instant::now()).unwrap();
    assert!(rejection.contains("timestamp drift"));
    assert!(cache.is_empty());
}

#[test]
fn replay_cache_key_binds_minion_connection_and_nonce() {
    let key = SysMaster::replay_cache_key_for_test(&SecureSessionBinding::bootstrap_opening(
        "mid-1".to_string(),
        "minion-fp".to_string(),
        "master-fp".to_string(),
        "conn-1".to_string(),
        "nonce-1".to_string(),
        fresh_timestamp(),
    ));

    assert_eq!(key, "mid-1:conn-1:nonce-1");
    assert_eq!(SysMaster::peer_rate_limit_key_for_test("127.0.0.1:4200"), "127.0.0.1");
}

#[test]
fn invalid_hello_does_not_poison_replay_cache_then_valid_retry_is_accepted() {
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (minion_prk, minion_pbk) = keygen(2048).unwrap();
    let state = state(&master_pbk, &minion_pbk);
    let (_, opening) = SecureBootstrapSession::open(&state, &minion_prk, &master_pbk).unwrap();
    let hello = match opening {
        SecureFrame::BootstrapHello(hello) => hello,
        _ => panic!("expected bootstrap hello"),
    };

    let mut tampered = hello.clone();
    tampered.binding_signature = "corrupted-signature".to_string();

    let mut cache = HashMap::<String, Instant>::new();
    let replay_key = SysMaster::replay_cache_key_for_test(&hello.binding);

    assert!(SysMaster::accept_bootstrap_auth_then_replay_for_test(&mut cache, &state, &tampered, &master_prk, &minion_pbk, Instant::now(),).is_err());
    assert!(!cache.contains_key(&replay_key));

    assert!(matches!(
        SysMaster::accept_bootstrap_auth_then_replay_for_test(&mut cache, &state, &hello, &master_prk, &minion_pbk, Instant::now(),).unwrap(),
        SecureFrame::BootstrapAck(_)
    ));

    assert!(matches!(
        SysMaster::accept_bootstrap_auth_then_replay_for_test(
            &mut cache,
            &state,
            &hello,
            &master_prk,
            &minion_pbk,
            Instant::now(),
        )
        .unwrap(),
        SecureFrame::BootstrapDiagnostic(frame) if frame.message.contains("replay")
    ));
}
