use chrono::{Duration, Utc};
use libsysinspect::{
    rsa::keys::{get_fingerprint, keygen},
    rsa::rotation::{RotationActor, RsaTransportRotator},
    transport::{TransportKeyStatus, TransportRotationStatus, TransportStore},
};

#[test]
fn rotation_execute_persists_new_active_key_and_purges_old_material() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("transport/minions/mid-1/state.json");
    let store = TransportStore::new(store_path.clone()).unwrap();

    let rotator = RsaTransportRotator::new(RotationActor::Master, store, "mid-1", "master-fp", "minion-fp", 1).unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-old", TransportKeyStatus::Active);
    let old = Utc::now() - Duration::days(3);
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-old") {
        record.created_at = old;
        record.activated_at = Some(old);
    }
    state.updated_at = old;
    TransportStore::new(store_path.clone()).unwrap().save(&state).unwrap();

    let mut rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-1", "master-fp", "minion-fp", 1)
            .unwrap();

    let plan = rotator.plan("scheduled");
    let ticket = rotator.execute(&plan).unwrap();

    let persisted = TransportStore::new(store_path).unwrap().load().unwrap().unwrap();
    assert_eq!(persisted.active_key_id.as_deref(), Some(plan.next_key_id()));
    assert_eq!(persisted.keys.len(), 1);
    assert_eq!(persisted.keys[0].status, TransportKeyStatus::Active);
    assert!(ticket.result().purged_keys() >= 1);
}

#[test]
fn pending_rotation_survives_reload_and_clears_after_execute() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("transport/minions/mid-2/state.json");

    let rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-2", "master-fp", "minion-fp", 1)
            .unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-current", TransportKeyStatus::Active);
    let old = Utc::now() - Duration::days(5);
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-current") {
        record.created_at = old;
        record.activated_at = Some(old);
    }
    state.updated_at = old;
    TransportStore::new(store_path.clone()).unwrap().save(&state).unwrap();

    let mut rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-2", "master-fp", "minion-fp", 1)
            .unwrap();

    assert!(rotator.queue_if_due(Duration::hours(48), Utc::now()).unwrap());

    let reloaded =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-2", "master-fp", "minion-fp", 1)
            .unwrap();
    assert_eq!(reloaded.state().rotation, TransportRotationStatus::Pending);

    let plan = reloaded.plan("pending-online");
    let mut reloaded = reloaded;
    let _ = reloaded.execute(&plan).unwrap();
    assert_eq!(reloaded.state().rotation, TransportRotationStatus::Idle);
}

#[test]
fn rollback_restores_previous_state_on_failure_path() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("transport/minions/mid-3/state.json");

    let rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-3", "master-fp", "minion-fp", 1)
            .unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-base", TransportKeyStatus::Active);
    TransportStore::new(store_path.clone()).unwrap().save(&state).unwrap();

    let mut rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-3", "master-fp", "minion-fp", 1)
            .unwrap();

    let previous = rotator.state().clone();
    let plan = rotator.plan("manual");
    let ticket = rotator.execute(&plan).unwrap();
    let restored = rotator.rollback(&ticket).unwrap();

    assert_eq!(restored.active_key_id, previous.active_key_id);
    assert_eq!(restored.keys, previous.keys);

    let persisted = TransportStore::new(store_path).unwrap().load().unwrap().unwrap();
    assert_eq!(persisted.active_key_id, previous.active_key_id);
    assert_eq!(persisted.keys, previous.keys);
}

#[test]
fn execute_signed_intent_rotates_only_when_rsa_trust_anchor_verifies() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("transport/minions/mid-4/state.json");
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let master_fp = get_fingerprint(&master_pbk).unwrap();

    let rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-4", &master_fp, "minion-fp", 1)
            .unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-old", TransportKeyStatus::Active);
    TransportStore::new(store_path.clone()).unwrap().save(&state).unwrap();

    let mut rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-4", &master_fp, "minion-fp", 1)
            .unwrap();

    let plan = rotator.plan("scheduled");
    let signed = rotator.sign_plan(&plan, &master_prk).unwrap();
    let _ = rotator.execute_signed_intent(&signed, &master_pbk).unwrap();

    let persisted = TransportStore::new(store_path).unwrap().load().unwrap().unwrap();
    assert_eq!(persisted.active_key_id.as_deref(), Some(plan.next_key_id()));
    assert_eq!(persisted.keys.len(), 1);
}

#[test]
fn execute_signed_intent_with_overlap_persists_retiring_key_until_grace_expires() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("transport/minions/mid-5/state.json");
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let master_fp = get_fingerprint(&master_pbk).unwrap();

    let rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-5", &master_fp, "minion-fp", 1)
            .unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-old", TransportKeyStatus::Active);
    TransportStore::new(store_path.clone()).unwrap().save(&state).unwrap();

    let mut rotator =
        RsaTransportRotator::new(RotationActor::Master, TransportStore::new(store_path.clone()).unwrap(), "mid-5", &master_fp, "minion-fp", 1)
            .unwrap();

    let plan = rotator.plan("scheduled");
    let signed = rotator.sign_plan(&plan, &master_prk).unwrap();
    let _ = rotator.execute_signed_intent_with_overlap(&signed, &master_pbk, Duration::hours(1)).unwrap();

    let persisted = TransportStore::new(store_path.clone()).unwrap().load().unwrap().unwrap();
    assert!(persisted.keys.iter().any(|key| key.key_id == "kid-old" && key.status == TransportKeyStatus::Retiring));

    let _ = rotator.retire_elapsed_keys(Utc::now() + Duration::hours(2), Duration::hours(1)).unwrap();
    let persisted = TransportStore::new(store_path).unwrap().load().unwrap().unwrap();
    assert!(persisted.keys.iter().all(|key| key.key_id != "kid-old"));
}
