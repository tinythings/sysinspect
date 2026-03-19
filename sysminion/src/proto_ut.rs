use crate::proto::msg::payload_to_diag;
use libsysproto::secure::{SecureBootstrapDiagnostic, SecureDiagnosticCode, SecureFailureSemantics, SecureFrame};

#[test]
fn payload_to_diag_reads_bootstrap_diagnostic_frames() {
    let diag = payload_to_diag(
        &serde_json::to_vec(&SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
            code: SecureDiagnosticCode::UnsupportedVersion,
            message: "old peer".to_string(),
            failure: SecureFailureSemantics::diagnostic(true, false),
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(matches!(diag.code, SecureDiagnosticCode::UnsupportedVersion));
    assert_eq!(diag.message, "old peer");
}

#[test]
fn payload_to_diag_rejects_non_diagnostic_frames() {
    assert!(payload_to_diag(br#"{"id":"mid-1","r":"ehlo","d":{},"c":1,"sid":""}"#).is_err());
}
