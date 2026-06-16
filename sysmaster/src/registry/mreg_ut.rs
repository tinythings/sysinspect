use super::MinionRegistry;
use crate::registry::rec::MinionCmdbStartup;
use chrono::Utc;
use libsysproto::MinionTarget;
use serde_json::json;
use std::collections::{BTreeSet, HashMap};

fn registry_with_one_minion() -> MinionRegistry {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();
    let mut traits = HashMap::new();
    traits.insert("system.hostname".to_string(), json!("alien"));
    traits.insert("system.hostname.fqdn".to_string(), json!("alien.lab"));
    traits.insert("system.hostname.ip".to_string(), json!("192.168.2.186"));
    registry.refresh("30006546535e428aba0a0caa6712e225", traits, BTreeSet::new(), BTreeSet::new()).unwrap();
    registry
}

// ---------------------------------------------------------------------------
//  Upgrade marker persistence
// ---------------------------------------------------------------------------

#[test]
fn mark_upgrade_required_stores_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "abcdef123456").unwrap();

    let marker = registry.get_upgrade_marker("mid-1").unwrap().unwrap();
    assert_eq!(marker.checksum(), "abcdef123456");
    assert!(!marker.unreachable);
    assert!(registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn clear_upgrade_required_removes_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "abc").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());

    registry.clear_upgrade_required("mid-1").unwrap();
    assert!(!registry.is_upgrade_required("mid-1").unwrap());
    assert!(registry.get_upgrade_marker("mid-1").unwrap().is_none());
}

#[test]
fn is_upgrade_required_true_false_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    // Never marked
    assert!(!registry.is_upgrade_required("ghost").unwrap());

    // Marked
    registry.mark_upgrade_required("mid-1", "0.5.0", "sha").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());

    // Cleared
    registry.clear_upgrade_required("mid-1").unwrap();
    assert!(!registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn mark_upgrade_required_overwrites_previous() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.4.0", "old-sha").unwrap();
    assert_eq!(registry.get_upgrade_marker("mid-1").unwrap().unwrap().checksum(), "old-sha");

    registry.mark_upgrade_required("mid-1", "0.5.0", "new-sha").unwrap();
    assert_eq!(registry.get_upgrade_marker("mid-1").unwrap().unwrap().checksum(), "new-sha");
}

#[test]
fn marker_survives_registry_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().to_path_buf();

    {
        let registry = MinionRegistry::new(db_path.clone()).unwrap();
        registry.mark_upgrade_required("mid-1", "0.5.0", "persist-sha").unwrap();
    }

    let registry = MinionRegistry::new(db_path).unwrap();
    let marker = registry.get_upgrade_marker("mid-1").unwrap().unwrap();
    assert_eq!(marker.checksum(), "persist-sha");
    assert!(!marker.unreachable);
}

// ---------------------------------------------------------------------------
//  Status counts and ID listing
// ---------------------------------------------------------------------------

#[test]
fn upgrade_status_counts_empty_registry() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    assert_eq!(registry.upgrade_status_counts().unwrap(), (0, 0));
}

#[test]
fn upgrade_status_counts_multiple_minions() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha-1").unwrap();
    registry.mark_upgrade_required("mid-2", "0.5.0", "sha-2").unwrap();
    registry.mark_upgrade_required("mid-3", "0.5.0", "sha-3").unwrap();
    registry.mark_upgrade_unreachable("mid-2").unwrap();

    let (required, unreachable) = registry.upgrade_status_counts().unwrap();
    assert_eq!(required, 3);
    assert_eq!(unreachable, 1);

    let marker = registry.get_upgrade_marker("mid-2").unwrap().unwrap();
    assert!(marker.unreachable);
}

#[test]
fn get_upgrade_required_ids_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("zulu", "0.5.0", "sha").unwrap();
    registry.mark_upgrade_required("alpha", "0.5.0", "sha").unwrap();
    registry.mark_upgrade_required("charlie", "0.5.0", "sha").unwrap();

    assert_eq!(registry.get_upgrade_required_ids().unwrap(), vec!["alpha", "charlie", "zulu"]);
}

#[test]
fn get_upgrade_required_ids_empty_after_clear() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha").unwrap();
    assert!(!registry.get_upgrade_required_ids().unwrap().is_empty());

    registry.clear_upgrade_required("mid-1").unwrap();
    assert!(registry.get_upgrade_required_ids().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
//  Unreachable flag
// ---------------------------------------------------------------------------

#[test]
fn mark_upgrade_unreachable_sets_flag_on_existing_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha").unwrap();
    registry.mark_upgrade_unreachable("mid-1").unwrap();

    let marker = registry.get_upgrade_marker("mid-1").unwrap().unwrap();
    assert!(marker.unreachable);
    assert_eq!(registry.upgrade_status_counts().unwrap(), (1, 1));
}

#[test]
fn mark_upgrade_unreachable_noop_for_unknown_mid() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    // No marker exists — should not error
    registry.mark_upgrade_unreachable("ghost").unwrap();
    assert_eq!(registry.upgrade_status_counts().unwrap(), (0, 0));
}

// ---------------------------------------------------------------------------
//  Clear all markers
// ---------------------------------------------------------------------------

#[test]
fn clear_all_upgrade_markers_empties_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha").unwrap();
    registry.mark_upgrade_required("mid-2", "0.5.0", "sha").unwrap();
    registry.mark_upgrade_required("mid-3", "0.5.0", "sha").unwrap();

    registry.clear_all_upgrade_markers().unwrap();

    assert_eq!(registry.upgrade_status_counts().unwrap(), (0, 0));
    assert!(registry.get_upgrade_required_ids().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
//  Checksum-match auto-clear
// ---------------------------------------------------------------------------

#[test]
fn clear_upgrade_if_checksum_matches_clears_on_exact_match() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let mut traits = HashMap::new();
    traits.insert("minion.binary.sha256".to_string(), json!("abcdef123456"));
    registry.refresh("mid-1", traits.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "abcdef123456").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());

    registry.clear_upgrade_required_if_checksum_matches("mid-1", &traits).unwrap();
    assert!(!registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn clear_upgrade_if_checksum_matches_keeps_on_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let mut traits = HashMap::new();
    traits.insert("minion.binary.sha256".to_string(), json!("old-minion-sha"));
    registry.refresh("mid-1", traits.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "new-repo-sha").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());

    // Different SHA — marker stays
    registry.clear_upgrade_required_if_checksum_matches("mid-1", &traits).unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn clear_upgrade_if_checksum_matches_skips_without_trait() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let traits_without_sha: HashMap<String, serde_json::Value> = HashMap::new();
    registry.refresh("mid-1", traits_without_sha.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());

    // No `minion.binary.sha256` trait — should not clear
    registry.clear_upgrade_required_if_checksum_matches("mid-1", &traits_without_sha).unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn clear_upgrade_if_checksum_matches_skips_without_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let mut traits = HashMap::new();
    traits.insert("minion.binary.sha256".to_string(), json!("sha"));
    registry.refresh("mid-1", traits.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    // No marker exists — should not error
    registry.clear_upgrade_required_if_checksum_matches("mid-1", &traits).unwrap();
    assert!(!registry.is_upgrade_required("mid-1").unwrap());
}

#[test]
fn clear_upgrade_if_checksum_matches_preserves_other_minions() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let mut traits_1 = HashMap::new();
    traits_1.insert("minion.binary.sha256".to_string(), json!("sha-1"));
    registry.refresh("mid-1", traits_1.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    let mut traits_2 = HashMap::new();
    traits_2.insert("minion.binary.sha256".to_string(), json!("sha-2"));
    registry.refresh("mid-2", traits_2.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    registry.mark_upgrade_required("mid-1", "0.5.0", "sha-1").unwrap();
    registry.mark_upgrade_required("mid-2", "0.5.0", "sha-2").unwrap();

    // Only mid-1 matches
    registry.clear_upgrade_required_if_checksum_matches("mid-1", &traits_1).unwrap();

    assert!(!registry.is_upgrade_required("mid-1").unwrap());
    assert!(registry.is_upgrade_required("mid-2").unwrap());
    assert_eq!(registry.upgrade_status_counts().unwrap(), (1, 0));
}

// ---------------------------------------------------------------------------
//  Full lifecycle
// ---------------------------------------------------------------------------

#[test]
fn full_upgrade_lifecycle_register_mark_simulate_upgrade_autoclear() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    // Minion registers with old binary
    let mut old_traits = HashMap::new();
    old_traits.insert("system.hostname".to_string(), json!("alien"));
    old_traits.insert("system.os.name".to_string(), json!("Linux"));
    old_traits.insert("system.arch".to_string(), json!("x86_64"));
    old_traits.insert("minion.version".to_string(), json!("0.4.0"));
    old_traits.insert("minion.binary.sha256".to_string(), json!("old-sha-256"));
    registry.refresh("mid-1", old_traits, BTreeSet::new(), BTreeSet::new()).unwrap();

    // Platform added to repo — master marks upgrade
    registry.mark_upgrade_required("mid-1", "0.5.0", "new-sha-256").unwrap();
    assert!(registry.is_upgrade_required("mid-1").unwrap());
    assert_eq!(registry.upgrade_status_counts().unwrap(), (1, 0));

    // Minion self-upgrades, restarts — sends new traits
    let mut new_traits = HashMap::new();
    new_traits.insert("minion.binary.sha256".to_string(), json!("new-sha-256"));
    registry.refresh("mid-1", new_traits.clone(), BTreeSet::new(), BTreeSet::new()).unwrap();

    // Master auto-clears on checksum match
    registry.clear_upgrade_required_if_checksum_matches("mid-1", &new_traits).unwrap();
    assert!(!registry.is_upgrade_required("mid-1").unwrap());
    assert_eq!(registry.upgrade_status_counts().unwrap(), (0, 0));
}

#[test]
fn get_upgrade_marker_returns_none_for_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    assert!(registry.get_upgrade_marker("nobody").unwrap().is_none());
}
