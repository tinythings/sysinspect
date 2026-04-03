use crate::rsa::MinionRSAKeyManager;
use libsysinspect::{
    cfg::mmconf::CFG_MASTER_KEY_PUB,
    rsa::keys::{RsaKey::Public, get_fingerprint, key_to_file, keygen, to_pem},
};

#[test]
fn ensure_transport_state_is_noop_without_master_key() {
    let root = tempfile::tempdir().unwrap();
    let keyman = MinionRSAKeyManager::new(root.path().to_path_buf()).unwrap();

    assert!(!keyman.ensure_transport_state("mid-1").unwrap());
    assert!(!root.path().join("transport/master/state.json").exists());
}

#[test]
fn ensure_transport_state_writes_managed_state_when_master_key_exists() {
    let root = tempfile::tempdir().unwrap();
    let keyman = MinionRSAKeyManager::new(root.path().to_path_buf()).unwrap();
    let (_, master_pbk) = keygen(2048).unwrap();

    key_to_file(&Public(master_pbk), root.path().to_str().unwrap(), CFG_MASTER_KEY_PUB).unwrap();

    assert!(keyman.ensure_transport_state("mid-1").unwrap());
    assert!(root.path().join("transport/master/state.json").exists());
}

#[test]
fn trust_master_identity_persists_master_key_and_transport_state() {
    let root = tempfile::tempdir().unwrap();
    let keyman = MinionRSAKeyManager::new(root.path().to_path_buf()).unwrap();
    let (_, master_pbk) = keygen(2048).unwrap();
    let (_, master_pem) = to_pem(None, Some(&master_pbk)).unwrap();
    let fp = get_fingerprint(&master_pbk).unwrap();

    assert_eq!(keyman.trust_master_identity("mid-1", &master_pem.unwrap(), Some(&fp)).unwrap(), fp);
    assert!(root.path().join(CFG_MASTER_KEY_PUB).exists());
    assert!(root.path().join("transport/master/state.json").exists());
}

#[test]
fn trust_master_identity_rejects_pinned_fingerprint_mismatch() {
    let root = tempfile::tempdir().unwrap();
    let keyman = MinionRSAKeyManager::new(root.path().to_path_buf()).unwrap();
    let (_, master_pbk) = keygen(2048).unwrap();
    let (_, master_pem) = to_pem(None, Some(&master_pbk)).unwrap();

    let err = keyman.trust_master_identity("mid-1", &master_pem.unwrap(), Some("deadbeef")).unwrap_err().to_string();
    assert!(err.contains("mismatch"));
}
