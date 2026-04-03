use crate::registry::mkb::{MinionsKeyRegistry, RegistrationStatus};
use libsysinspect::{
    rsa::keys::{keygen, to_pem},
    transport::TransportStore,
};
use libsysproto::secure::SECURE_PROTOCOL_VERSION;

#[test]
fn registration_creates_transport_state_for_registered_minion() {
    let root = tempfile::tempdir().unwrap();
    let mut reg = MinionsKeyRegistry::new(root.path().join("minion-keys")).unwrap();
    let (_, pbk) = keygen(2048).unwrap();
    let (_, pem) = to_pem(None, Some(&pbk)).unwrap();

    assert_eq!(reg.add_mn_key("mid-1", "127.0.0.1:4200", &pem.unwrap()).unwrap(), RegistrationStatus::Added);

    let store = TransportStore::new(root.path().join("transport/minions/mid-1/state.json")).unwrap();
    let state = store.load().unwrap().unwrap();
    assert_eq!(state.minion_id, "mid-1");
    assert_eq!(state.protocol_version, SECURE_PROTOCOL_VERSION);
    assert_eq!(state.master_rsa_fingerprint, reg.get_master_key_fingerprint().unwrap());
    assert_eq!(state.minion_rsa_fingerprint, reg.get_mn_key_fingerprint("mid-1").unwrap());
}

#[test]
fn startup_backfills_transport_state_for_existing_registered_minion() {
    let root = tempfile::tempdir().unwrap();
    let (_, pbk) = keygen(2048).unwrap();
    let (_, pem) = to_pem(None, Some(&pbk)).unwrap();
    std::fs::create_dir_all(root.path().join("minion-keys")).unwrap();
    std::fs::write(root.path().join("minion-keys/mid-1.rsa.pub"), pem.unwrap()).unwrap();

    let mut reg = MinionsKeyRegistry::new(root.path().join("minion-keys")).unwrap();
    let store = TransportStore::new(root.path().join("transport/minions/mid-1/state.json")).unwrap();
    let state = store.load().unwrap().unwrap();
    assert_eq!(state.minion_id, "mid-1");
    assert_eq!(state.master_rsa_fingerprint, reg.get_master_key_fingerprint().unwrap());
    assert_eq!(state.minion_rsa_fingerprint, reg.get_mn_key_fingerprint("mid-1").unwrap());
}

#[test]
fn registration_rejects_key_mismatch_for_existing_minion() {
    let root = tempfile::tempdir().unwrap();
    let mut reg = MinionsKeyRegistry::new(root.path().join("minion-keys")).unwrap();
    let (_, first_pbk) = keygen(2048).unwrap();
    let (_, first_pem) = to_pem(None, Some(&first_pbk)).unwrap();
    let (_, second_pbk) = keygen(2048).unwrap();
    let (_, second_pem) = to_pem(None, Some(&second_pbk)).unwrap();

    assert_eq!(reg.add_mn_key("mid-1", "127.0.0.1:4200", &first_pem.unwrap()).unwrap(), RegistrationStatus::Added);
    assert!(matches!(reg.add_mn_key("mid-1", "127.0.0.1:4200", &second_pem.unwrap()).unwrap(), RegistrationStatus::Conflict { .. }));
}
