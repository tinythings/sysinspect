use crate::rsa::MinionRSAKeyManager;
use libsysproto::payload::RegistrationReply;
use tempfile::tempdir;

#[test]
fn accepted_registration_reply_exposes_trust_material() {
    let root = tempdir().unwrap();
    let keyman = MinionRSAKeyManager::new(root.path().to_path_buf()).unwrap();
    let fp = keyman.get_pubkey_fingerprint().unwrap();
    let reply = RegistrationReply::accepted("ok".to_string(), "pem".to_string(), fp.clone());

    assert!(reply.accepted_flag());
    assert_eq!(reply.message(), "ok");
    assert_eq!(reply.master_key_pem(), Some("pem"));
    assert_eq!(reply.master_fingerprint(), Some(fp.as_str()));
}

#[test]
fn rejected_registration_reply_has_no_trust_material() {
    let reply = RegistrationReply::rejected("nope".to_string());

    assert!(!reply.accepted_flag());
    assert_eq!(reply.master_key_pem(), None);
    assert_eq!(reply.master_fingerprint(), None);
}
