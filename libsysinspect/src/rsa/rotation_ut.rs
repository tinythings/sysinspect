use super::{RotationActor, RsaTransportRotator};
use crate::rsa::keys::keygen;
use crate::transport::{TransportKeyStatus, TransportStore};
use chrono::{Duration, Utc};

fn init_rotator() -> (tempfile::TempDir, RsaTransportRotator) {
    let tmp = tempfile::tempdir().unwrap();
    let store = TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap();
    let rotator = RsaTransportRotator::new(RotationActor::Master, store, "mid-1", "master-fp", "minion-fp", 1).unwrap();

    let mut state = rotator.state().clone();
    state.upsert_key("kid-old", TransportKeyStatus::Active);
    let old = Utc::now() - Duration::days(5);
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-old") {
        record.created_at = old;
        record.activated_at = Some(old);
    }
    state.updated_at = old;
    state.rotation = crate::transport::TransportRotationStatus::Idle;
    TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();

    let rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        "master-fp",
        "minion-fp",
        1,
    )
    .unwrap();

    (tmp, rotator)
}

#[test]
fn plan_generates_fingerprint_for_new_key() {
    let (_tmp, rotator) = init_rotator();
    let plan = rotator.plan("scheduled");

    assert_eq!(plan.minion_id(), "mid-1");
    assert!(plan.next_key_id().starts_with("trk-"));
    assert_eq!(plan.next_key_fingerprint(), RsaTransportRotator::fingerprint_for_key_id(plan.next_key_id()));
}

#[test]
fn execute_activates_new_key_and_purges_old_keys() {
    let (_tmp, mut rotator) = init_rotator();
    let plan = rotator.plan("manual");
    let ticket = rotator.execute(&plan).unwrap();

    assert_eq!(ticket.result().active_key_id(), plan.next_key_id());
    assert_eq!(rotator.state().active_key_id.as_deref(), Some(plan.next_key_id()));
    assert_eq!(rotator.state().keys.len(), 1);
    assert_eq!(rotator.state().keys[0].status, TransportKeyStatus::Active);
    assert!(ticket.result().purged_keys() >= 1);
}

#[test]
fn execute_with_overlap_keeps_retiring_key_until_window_expires() {
    let (tmp, mut rotator) = init_rotator();
    let mut state = rotator.state().clone();
    let now = Utc::now();
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-old") {
        record.created_at = now;
        record.activated_at = Some(now);
    }
    TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();
    rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        "master-fp",
        "minion-fp",
        1,
    )
    .unwrap();

    let plan = rotator.plan("manual");
    let ticket = rotator.execute_with_overlap(&plan, Duration::hours(1)).unwrap();

    assert_eq!(ticket.result().active_key_id(), plan.next_key_id());
    assert!(rotator.state().keys.len() >= 2);
    assert!(rotator.state().keys.iter().any(|k| k.key_id == plan.next_key_id() && k.status == TransportKeyStatus::Active));
    assert!(rotator.state().keys.iter().any(|k| k.key_id == "kid-old" && k.status == TransportKeyStatus::Retiring));
}

#[test]
fn retire_elapsed_keys_purges_retired_material_after_overlap() {
    let (tmp, mut rotator) = init_rotator();
    let mut state = rotator.state().clone();
    let now = Utc::now();
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-old") {
        record.created_at = now;
        record.activated_at = Some(now);
    }
    TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();
    rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        "master-fp",
        "minion-fp",
        1,
    )
    .unwrap();

    let plan = rotator.plan("manual");
    let _ = rotator.execute_with_overlap(&plan, Duration::hours(2)).unwrap();

    let purged = rotator.retire_elapsed_keys(Utc::now() + Duration::hours(3), Duration::hours(2)).unwrap();

    assert!(purged >= 1);
    assert_eq!(rotator.state().active_key_id.as_deref(), Some(plan.next_key_id()));
    assert!(rotator.state().keys.iter().all(|key| !matches!(key.status, TransportKeyStatus::Retired)));
}

#[test]
fn rollback_restores_previous_state() {
    let (_tmp, mut rotator) = init_rotator();
    let previous = rotator.state().clone();
    let plan = rotator.plan("manual");
    let ticket = rotator.execute(&plan).unwrap();

    let restored = rotator.rollback(&ticket).unwrap();

    assert_eq!(restored.active_key_id, previous.active_key_id);
    assert_eq!(restored.keys, previous.keys);
}

#[test]
fn queue_if_due_sets_pending_rotation() {
    let (_tmp, mut rotator) = init_rotator();
    let queued = rotator.queue_if_due(Duration::hours(48), Utc::now()).unwrap();

    assert!(queued);
    assert_eq!(rotator.state().rotation, crate::transport::TransportRotationStatus::Pending);
}

#[test]
fn signed_rotation_intent_verifies_against_master_trust_anchor() {
    let (_tmp, rotator) = init_rotator();
    let (master_prk, master_pbk) = keygen(2048).unwrap();

    let mut state = rotator.state().clone();
    state.master_rsa_fingerprint = crate::rsa::keys::get_fingerprint(&master_pbk).unwrap();
    TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();
    let rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        &crate::rsa::keys::get_fingerprint(&master_pbk).unwrap(),
        "minion-fp",
        1,
    )
    .unwrap();

    let plan = rotator.plan("manual");
    let signed = rotator.sign_plan(&plan, &master_prk).unwrap();

    rotator.verify_signed_intent(&signed, &master_pbk).unwrap();
}

#[test]
fn minion_side_signed_rotation_intent_verifies_against_master_trust_anchor() {
    let (_tmp, rotator) = init_rotator();
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (_minion_prk, minion_pbk) = keygen(2048).unwrap();

    let master_fp = crate::rsa::keys::get_fingerprint(&master_pbk).unwrap();
    let minion_fp = crate::rsa::keys::get_fingerprint(&minion_pbk).unwrap();
    let mut state = rotator.state().clone();
    state.master_rsa_fingerprint = master_fp.clone();
    state.minion_rsa_fingerprint = minion_fp.clone();
    TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();

    let master_rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        &master_fp,
        &minion_fp,
        1,
    )
    .unwrap();
    let signed = master_rotator.sign_plan(&master_rotator.plan("manual"), &master_prk).unwrap();

    let minion_rotator = RsaTransportRotator::new(
        RotationActor::Minion,
        TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        &master_fp,
        &minion_fp,
        1,
    )
    .unwrap();

    minion_rotator.verify_signed_intent(&signed, &master_pbk).unwrap();
}

#[test]
fn signed_rotation_intent_rejects_wrong_signer() {
    let (_tmp, rotator) = init_rotator();
    let (master_prk, master_pbk) = keygen(2048).unwrap();
    let (_other_prk, other_pbk) = keygen(2048).unwrap();

    let mut state = rotator.state().clone();
    state.master_rsa_fingerprint = crate::rsa::keys::get_fingerprint(&master_pbk).unwrap();
    TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();
    let rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(_tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        &crate::rsa::keys::get_fingerprint(&master_pbk).unwrap(),
        "minion-fp",
        1,
    )
    .unwrap();

    let signed = rotator.sign_plan(&rotator.plan("manual"), &master_prk).unwrap();

    assert!(rotator.verify_signed_intent(&signed, &other_pbk).is_err());
}

#[test]
fn execute_signed_intent_with_overlap_keeps_retiring_key_until_window_expires() {
    let (tmp, mut rotator) = init_rotator();
    let (master_prk, master_pbk) = keygen(2048).unwrap();

    let mut state = rotator.state().clone();
    let now = Utc::now();
    state.master_rsa_fingerprint = crate::rsa::keys::get_fingerprint(&master_pbk).unwrap();
    if let Some(record) = state.keys.iter_mut().find(|record| record.key_id == "kid-old") {
        record.created_at = now;
        record.activated_at = Some(now);
    }
    TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap().save(&state).unwrap();
    rotator = RsaTransportRotator::new(
        RotationActor::Master,
        TransportStore::new(tmp.path().join("transport/minions/mid-1/state.json")).unwrap(),
        "mid-1",
        &crate::rsa::keys::get_fingerprint(&master_pbk).unwrap(),
        "minion-fp",
        1,
    )
    .unwrap();

    let plan = rotator.plan("manual");
    let signed = rotator.sign_plan(&plan, &master_prk).unwrap();
    let _ = rotator.execute_signed_intent_with_overlap(&signed, &master_pbk, Duration::hours(1)).unwrap();

    assert!(rotator.state().keys.iter().any(|k| k.key_id == plan.next_key_id() && k.status == TransportKeyStatus::Active));
    assert!(rotator.state().keys.iter().any(|k| k.key_id == "kid-old" && k.status == TransportKeyStatus::Retiring));
}
