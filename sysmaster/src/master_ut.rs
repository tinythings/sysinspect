use crate::master::SysMaster;
use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SecureBootstrapHello, SecureFrame, SecureSessionBinding};
use std::{collections::HashMap, time::Instant};

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
            ),
            session_key_cipher: "cipher".to_string(),
            binding_signature: "sig".to_string(),
            key_id: Some("kid-1".to_string()),
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&bounced).unwrap(),
        SecureFrame::BootstrapDiagnostic(_)
    ));
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
    let bounced = SysMaster::secure_peer_diag_with_state(&mut failures, "127.0.0.1:4200", br#"{"kind":"bootstrap_hello","broken":}"#)
        .unwrap();

    assert!(matches!(
        serde_json::from_slice::<SecureFrame>(&bounced).unwrap(),
        SecureFrame::BootstrapDiagnostic(frame) if frame.message.contains("Repeated malformed")
    ));
}
