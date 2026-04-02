use super::{
    SECURE_PROTOCOL_VERSION, SECURE_SUPPORTED_PROTOCOL_VERSIONS, SecureBootstrapAck, SecureBootstrapDiagnostic, SecureBootstrapHello,
    SecureDataFrame, SecureDiagnosticCode, SecureFailureSemantics, SecureFrame, SecureRotationMode, SecureSessionBinding, SecureTransportGoals,
};

fn binding() -> SecureSessionBinding {
    SecureSessionBinding {
        minion_id: "minion-a".to_string(),
        minion_rsa_fingerprint: "minion-fp".to_string(),
        master_rsa_fingerprint: "master-fp".to_string(),
        protocol_version: SECURE_PROTOCOL_VERSION,
        connection_id: "conn-1".to_string(),
        client_nonce: "client-nonce".to_string(),
        master_nonce: "master-nonce".to_string(),
        timestamp: 1672531200,
    }
}

#[test]
fn master_minion_goals_match_phase_one_decisions() {
    assert_eq!(
        SecureTransportGoals::master_minion(),
        SecureTransportGoals {
            no_tls_dependency: true,
            no_dns_dependency: true,
            reconnect_tolerant: true,
            bounded_frames: true,
            replay_protection: true,
            explicit_rotation: true,
            minimal_plaintext_bootstrap: true,
            reject_non_bootstrap_plaintext: true,
            single_active_session_per_minion: true,
        }
    );
}

#[test]
fn only_bootstrap_frames_may_stay_plaintext() {
    assert!(
        SecureFrame::BootstrapHello(SecureBootstrapHello {
            binding: binding(),
            supported_versions: SECURE_SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
            client_ephemeral_pubkey: "pubkey".to_string(),
            binding_signature: "sig".to_string(),
            key_id: None,
        })
        .is_plaintext_bootstrap()
    );
    assert!(
        SecureFrame::BootstrapAck(SecureBootstrapAck {
            binding: binding(),
            session_id: "sid".to_string(),
            key_id: "kid".to_string(),
            rotation: SecureRotationMode::None,
            master_ephemeral_pubkey: "pubkey".to_string(),
            binding_signature: "sig".to_string(),
        })
        .is_plaintext_bootstrap()
    );
    assert!(
        SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
            code: SecureDiagnosticCode::MalformedFrame,
            message: "bad".to_string(),
            failure: SecureFailureSemantics::diagnostic(false, true),
        })
        .is_plaintext_bootstrap()
    );
    assert!(
        !SecureFrame::Data(SecureDataFrame {
            protocol_version: SECURE_PROTOCOL_VERSION,
            session_id: "sid".to_string(),
            key_id: "kid".to_string(),
            counter: 1,
            nonce: "nonce".to_string(),
            payload: "payload".to_string(),
        })
        .is_plaintext_bootstrap()
    );
}

#[test]
fn data_frames_require_established_session_state() {
    assert!(
        SecureFrame::Data(SecureDataFrame {
            protocol_version: SECURE_PROTOCOL_VERSION,
            session_id: "sid".to_string(),
            key_id: "kid".to_string(),
            counter: 7,
            nonce: "nonce".to_string(),
            payload: "payload".to_string(),
        })
        .requires_established_session()
    );
    assert!(
        !SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
            code: SecureDiagnosticCode::UnsupportedVersion,
            message: "old peer".to_string(),
            failure: SecureFailureSemantics::diagnostic(true, false),
        })
        .requires_established_session()
    );
}

#[test]
fn secure_frame_serde_uses_stable_kind_tags() {
    assert_eq!(
        serde_json::to_value(SecureFrame::BootstrapHello(SecureBootstrapHello {
            binding: binding(),
            supported_versions: SECURE_SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
            client_ephemeral_pubkey: "pubkey".to_string(),
            binding_signature: "sig".to_string(),
            key_id: Some("kid".to_string()),
        }))
        .unwrap()["kind"],
        "bootstrap_hello"
    );
    assert_eq!(
        serde_json::to_value(SecureFrame::Data(SecureDataFrame {
            protocol_version: SECURE_PROTOCOL_VERSION,
            session_id: "sid".to_string(),
            key_id: "kid".to_string(),
            counter: 9,
            nonce: "nonce".to_string(),
            payload: "payload".to_string(),
        }))
        .unwrap()["kind"],
        "data"
    );
}

#[test]
fn secure_bootstrap_ack_roundtrips_through_json() {
    let frame = SecureFrame::BootstrapAck(SecureBootstrapAck {
        binding: binding(),
        session_id: "sid".to_string(),
        key_id: "kid".to_string(),
        rotation: SecureRotationMode::Rekey,
        master_ephemeral_pubkey: "pubkey".to_string(),
        binding_signature: "sig".to_string(),
    });

    let parsed = serde_json::from_slice::<SecureFrame>(&serde_json::to_vec(&frame).unwrap()).unwrap();

    assert_eq!(parsed, frame);
}

#[test]
fn secure_diagnostic_roundtrips_through_json() {
    let frame = SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
        code: SecureDiagnosticCode::ReplayRejected,
        message: "duplicate".to_string(),
        failure: SecureFailureSemantics::diagnostic(false, true),
    });

    let parsed = serde_json::from_slice::<SecureFrame>(&serde_json::to_vec(&frame).unwrap()).unwrap();

    assert_eq!(parsed, frame);
}

#[test]
fn secure_data_frame_roundtrips_through_json() {
    let frame = SecureFrame::Data(SecureDataFrame {
        protocol_version: SECURE_PROTOCOL_VERSION,
        session_id: "sid".to_string(),
        key_id: "kid".to_string(),
        counter: 7,
        nonce: "nonce".to_string(),
        payload: "payload".to_string(),
    });

    let parsed = serde_json::from_slice::<SecureFrame>(&serde_json::to_vec(&frame).unwrap()).unwrap();

    assert_eq!(parsed, frame);
}
