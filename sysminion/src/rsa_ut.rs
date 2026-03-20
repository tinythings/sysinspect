use crate::rsa::MinionRSAKeyManager;
use libsysinspect::{
    cfg::mmconf::CFG_MASTER_KEY_PUB,
    rsa::keys::{RsaKey::Public, key_to_file, keygen},
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
