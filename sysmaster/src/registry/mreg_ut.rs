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

#[test]
fn get_by_hostname_or_ip_matches_plain_hostname() {
    let mut registry = registry_with_one_minion();
    let records = registry.get_by_hostname_or_ip("alien").unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id(), "30006546535e428aba0a0caa6712e225");
}

#[test]
fn get_by_query_matches_plain_hostname() {
    let registry = registry_with_one_minion();
    let records = registry.get_by_query("alien").unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id(), "30006546535e428aba0a0caa6712e225");
}

#[tokio::test]
async fn targeted_minions_resolve_plain_hostname_from_id_slot() {
    let mut registry = registry_with_one_minion();
    let ids = registry.get_targeted_minions(&MinionTarget::new("alien", ""), true).await;

    assert_eq!(ids, vec!["30006546535e428aba0a0caa6712e225"]);
}

#[tokio::test]
async fn targeted_minions_resolve_partial_id_prefix_from_id_slot() {
    let mut registry = registry_with_one_minion();
    let ids = registry.get_targeted_minions(&MinionTarget::new("3000", ""), true).await;

    assert_eq!(ids, vec!["30006546535e428aba0a0caa6712e225"]);
}

#[tokio::test]
async fn targeted_minions_resolve_traits_query() {
    let mut registry = registry_with_one_minion();
    let mut target = MinionTarget::default();
    target.set_traits_query("system.hostname: alien");

    let ids = registry.get_targeted_minions(&target, true).await;

    assert_eq!(ids, vec!["30006546535e428aba0a0caa6712e225"]);
}

#[tokio::test]
async fn targeted_minions_require_hostname_and_traits_when_both_present() {
    let mut registry = registry_with_one_minion();
    let mut target = MinionTarget::default();
    target.add_hostname("alien*");
    target.set_traits_query("system.hostname: alien");

    let ids = registry.get_targeted_minions(&target, true).await;
    assert_eq!(ids, vec!["30006546535e428aba0a0caa6712e225"]);

    target.set_traits_query("system.hostname: wrong");
    assert!(registry.get_targeted_minions(&target, true).await.is_empty());
}

#[test]
fn cmdb_records_track_registered_startup_and_observed_host_data() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    registry.ensure_cmdb_registered("mid-1").unwrap();
    let base = registry.get_cmdb("mid-1").unwrap().unwrap();
    assert_eq!(base.mid(), "mid-1");
    assert_eq!(base.host(), None);

    registry
        .upsert_cmdb_startup(
            "mid-1",
            MinionCmdbStartup::new(
                "bo".to_string(),
                "192.168.122.105".to_string(),
                "/home/bo/sysinspect".to_string(),
                "/home/bo/sysinspect/bin/sysminion".to_string(),
                "/home/bo/sysinspect/etc/sysinspect.conf".to_string(),
                "hopstart".to_string(),
            ),
        )
        .unwrap();

    let mut traits = HashMap::new();
    traits.insert("system.hostname".to_string(), json!("demo"));
    traits.insert("system.hostname.fqdn".to_string(), json!("demo.lab"));
    traits.insert("system.hostname.ip".to_string(), json!("192.168.122.105"));
    registry.refresh_cmdb_observed("mid-1", &traits).unwrap();

    let cmdb = registry.get_cmdb("mid-1").unwrap().unwrap();
    assert_eq!(cmdb.user(), Some("bo"));
    assert_eq!(cmdb.host(), Some("192.168.122.105"));
    assert_eq!(cmdb.root(), Some("/home/bo/sysinspect"));
    assert_eq!(cmdb.bin(), Some("/home/bo/sysinspect/bin/sysminion"));
    assert_eq!(cmdb.config(), Some("/home/bo/sysinspect/etc/sysinspect.conf"));
    assert_eq!(cmdb.backend(), Some("hopstart"));
    assert_eq!(cmdb.hostname(), Some("demo"));
    assert_eq!(cmdb.fqdn(), Some("demo.lab"));
    assert_eq!(cmdb.ip(), Some("192.168.122.105"));

    registry.remove("mid-1").unwrap();
    assert!(registry.get_cmdb("mid-1").unwrap().is_none());
}

#[test]
fn stale_cmdb_reconcile_refreshes_host_facts_but_preserves_startup_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = MinionRegistry::new(tmp.path().to_path_buf()).unwrap();

    let mut traits = HashMap::new();
    traits.insert("system.hostname".to_string(), json!("demo"));
    traits.insert("system.hostname.fqdn".to_string(), json!("demo.lab"));
    traits.insert("system.hostname.ip".to_string(), json!("192.168.122.105"));
    registry.refresh("mid-1", traits, BTreeSet::new(), BTreeSet::new()).unwrap();
    registry.ensure_cmdb_registered("mid-1").unwrap();
    registry
        .upsert_cmdb_startup(
            "mid-1",
            MinionCmdbStartup::new(
                "bo".to_string(),
                "requested-host".to_string(),
                "/home/bo/sysinspect".to_string(),
                "/home/bo/sysinspect/bin/sysminion".to_string(),
                "/home/bo/sysinspect/etc/sysinspect.conf".to_string(),
                "hopstart".to_string(),
            ),
        )
        .unwrap();

    let mut stale = registry.get_cmdb("mid-1").unwrap().unwrap();
    stale.set_updated_at(Utc::now() - chrono::Duration::days(8));
    registry.add_cmdb("mid-1", &stale).unwrap();

    assert!(registry.reconcile_cmdb("mid-1", std::time::Duration::from_secs(7 * 24 * 60 * 60)).unwrap());

    let cmdb = registry.get_cmdb("mid-1").unwrap().unwrap();
    assert_eq!(cmdb.mid(), "mid-1");
    assert_eq!(cmdb.user(), Some("bo"));
    assert_eq!(cmdb.host(), Some("requested-host"));
    assert_eq!(cmdb.root(), Some("/home/bo/sysinspect"));
    assert_eq!(cmdb.bin(), Some("/home/bo/sysinspect/bin/sysminion"));
    assert_eq!(cmdb.config(), Some("/home/bo/sysinspect/etc/sysinspect.conf"));
    assert_eq!(cmdb.backend(), Some("hopstart"));
    assert_eq!(cmdb.hostname(), Some("demo"));
    assert_eq!(cmdb.fqdn(), Some("demo.lab"));
    assert_eq!(cmdb.ip(), Some("192.168.122.105"));
    assert!(!cmdb.is_stale(std::time::Duration::from_secs(7 * 24 * 60 * 60)));
}
