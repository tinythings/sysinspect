use super::{TransportKeyExchangeModel, TransportKeyStatus, TransportPeerState, TransportProvisioningMode, TransportStore, transport_minion_root};
use crate::cfg::mmconf::{MasterConfig, MinionConfig};

#[test]
fn minion_transport_store_uses_managed_state_path() {
    let mut cfg = MinionConfig::default();
    let root = tempfile::tempdir().unwrap();
    cfg.set_root_dir(root.path().to_str().unwrap());

    let store = TransportStore::for_minion(&cfg).unwrap();

    assert_eq!(store.state_path(), &cfg.transport_state_file());
}

#[test]
fn master_transport_store_rejects_traversing_minion_id() {
    let cfg = MasterConfig::default();

    assert!(TransportStore::for_master_minion(&cfg, "../escape").is_err());
}

#[test]
fn transport_state_roundtrips_through_managed_file() {
    let mut cfg = MinionConfig::default();
    let root = tempfile::tempdir().unwrap();
    cfg.set_root_dir(root.path().to_str().unwrap());
    let store = TransportStore::for_minion(&cfg).unwrap();
    let mut state = TransportPeerState::new("mid-1".to_string(), "master-fp".to_string(), "minion-fp".to_string(), 1);

    state.upsert_key("k1", TransportKeyStatus::Proposed);
    state.upsert_key("k1", TransportKeyStatus::Active);
    store.save(&state).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.minion_id, "mid-1");
    assert_eq!(loaded.active_key_id.as_deref(), Some("k1"));
    assert_eq!(loaded.last_key_id.as_deref(), Some("k1"));
    assert_eq!(loaded.keys.len(), 1);
    assert_eq!(loaded.keys[0].status, TransportKeyStatus::Active);
}

#[test]
fn transport_minion_root_uses_safe_peer_ids() {
    let root = tempfile::tempdir().unwrap();

    assert_eq!(
        transport_minion_root(root.path(), "node-1").unwrap(),
        root.path().join("minions").join("node-1")
    );
}

#[test]
fn transport_state_defaults_to_automatic_provisioning() {
    let state = TransportPeerState::new("mid-1".to_string(), "master-fp".to_string(), "minion-fp".to_string(), 1);

    assert_eq!(state.key_exchange, TransportKeyExchangeModel::EphemeralSessionKeys);
    assert_eq!(state.provisioning, TransportProvisioningMode::Automatic);
    assert!(state.approved_at.is_some());
}

#[test]
fn transport_state_can_switch_to_explicit_approval() {
    let mut state = TransportPeerState::new("mid-1".to_string(), "master-fp".to_string(), "minion-fp".to_string(), 1);

    state.set_provisioning(TransportProvisioningMode::ExplicitApproval);
    assert_eq!(state.provisioning, TransportProvisioningMode::ExplicitApproval);
    assert!(state.approved_at.is_none());
    state.approve();
    assert!(state.approved_at.is_some());
}

#[test]
fn store_can_require_explicit_approval_for_a_peer() {
    let root = tempfile::tempdir().unwrap();
    let store = super::TransportStore::new(root.path().join("transport/master/state.json")).unwrap();

    let state = store.require_explicit_approval("mid-1", "master-fp", "minion-fp", 1).unwrap();

    assert_eq!(state.provisioning, TransportProvisioningMode::ExplicitApproval);
    assert!(state.approved_at.is_none());
}

#[test]
fn store_can_approve_existing_peer_state() {
    let root = tempfile::tempdir().unwrap();
    let store = super::TransportStore::new(root.path().join("transport/master/state.json")).unwrap();

    store.require_explicit_approval("mid-1", "master-fp", "minion-fp", 1).unwrap();
    let approved = store.approve_peer().unwrap();

    assert_eq!(approved.provisioning, TransportProvisioningMode::ExplicitApproval);
    assert!(approved.approved_at.is_some());
}

#[test]
fn store_keeps_ephemeral_session_key_model_for_automatic_peers() {
    let root = tempfile::tempdir().unwrap();
    let store = super::TransportStore::new(root.path().join("transport/master/state.json")).unwrap();

    let state = store.ensure_automatic_peer("mid-1", "master-fp", "minion-fp", 1).unwrap();

    assert_eq!(state.key_exchange, TransportKeyExchangeModel::EphemeralSessionKeys);
}

#[cfg(unix)]
#[test]
fn transport_state_file_is_private_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let mut cfg = MinionConfig::default();
    let root = tempfile::tempdir().unwrap();
    cfg.set_root_dir(root.path().to_str().unwrap());
    let store = TransportStore::for_minion(&cfg).unwrap();

    store
        .save(&TransportPeerState::new(
            "mid-1".to_string(),
            "master-fp".to_string(),
            "minion-fp".to_string(),
            1,
        ))
        .unwrap();

    assert_eq!(std::fs::metadata(cfg.transport_master_root()).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(cfg.transport_state_file()).unwrap().permissions().mode() & 0o777, 0o600);
}
