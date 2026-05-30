use crate::master::SHARED_SESSION;

use super::rec::{MinionCmdbRecord, MinionCmdbStartup, MinionRecord};
use globset::Glob;
use libcommon::SysinspectError;
use libsysinspect::traits;
use libsysproto::MinionTarget;
use serde_json::{Value, json};
use sled::{Db, Tree};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
    sync::Arc,
};

const DB_MINIONS: &str = "minions";
const DB_CMDB: &str = "cmdb";

#[derive(Debug, Clone)]
pub struct MinionRegistry {
    conn: Db,
}

impl MinionRegistry {
    fn record_matches_target(record: &MinionRecord, target: &MinionTarget) -> bool {
        if !target.id().is_empty() {
            let id_match = record.id() == target.id()
                || record.id().starts_with(target.id())
                || ["system.hostname", "system.hostname.fqdn", "system.hostname.ip"]
                    .into_iter()
                    .filter_map(|key| record.get_traits().get(key).and_then(|value| value.as_str()))
                    .any(|label| Glob::new(target.id()).ok().is_some_and(|pattern| pattern.compile_matcher().is_match(label)));
            if !id_match {
                return false;
            }
        }

        if !target.hostnames().is_empty() {
            let labels = ["system.hostname", "system.hostname.fqdn", "system.hostname.ip"]
                .into_iter()
                .filter_map(|key| record.get_traits().get(key).and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            let hostname_match = target
                .hostnames()
                .into_iter()
                .any(|pattern| Glob::new(&pattern).ok().is_some_and(|glob| labels.iter().any(|label| glob.compile_matcher().is_match(label))));
            if !hostname_match {
                return false;
            }
        }

        if !target.traits_query().is_empty() {
            let query = match traits::parse_traits_query(target.traits_query()).and_then(traits::to_typed_query) {
                Ok(query) => query,
                Err(err) => {
                    log::error!("Unable to parse traits query '{}': {}", target.traits_query(), err);
                    return false;
                }
            };

            let mut or_matches = false;
            for and_group in query {
                let mut and_matches = true;
                for term in and_group {
                    let Some((key, expected)) = term.into_iter().next() else {
                        continue;
                    };
                    if record.get_traits().get(&key) != Some(&expected) {
                        and_matches = false;
                        break;
                    }
                }
                if and_matches {
                    or_matches = true;
                    break;
                }
            }

            if !or_matches {
                return false;
            }
        }

        true
    }

    pub fn new(pth: PathBuf) -> Result<MinionRegistry, SysinspectError> {
        if !pth.exists() {
            fs::create_dir_all(&pth)?;
        }

        Ok(MinionRegistry {
            conn: match sled::open(pth) {
                Ok(db) => db,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            },
        })
    }

    fn get_tree(&self, tid: &str) -> Result<Tree, SysinspectError> {
        let tree = self.conn.open_tree(tid);
        if let Err(err) = tree {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to open {tid} database: {err}")));
        }
        Ok(tree.unwrap())
    }

    /// Add or update traits
    pub fn refresh(
        &mut self, mid: &str, traits: HashMap<String, Value>, static_keys: BTreeSet<String>, fn_keys: BTreeSet<String>,
    ) -> Result<(), SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        match minions.contains_key(mid) {
            Ok(exists) => {
                if exists {
                    if let Err(err) = minions.remove(mid) {
                        return Err(SysinspectError::MasterGeneralError(format!(
                            "Unable to remove previous data for {mid} from the database: {err}"
                        )));
                    } else {
                        log::debug!("Traits for {mid} pre-removed");
                    }
                }
            }
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to access the database: {err}"))),
        };

        self.add(mid, MinionRecord::new(mid.to_string(), traits, static_keys, fn_keys))?;

        Ok(())
    }

    fn add(&mut self, mid: &str, mrec: MinionRecord) -> Result<(), SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        if let Err(err) = minions.insert(mid, json!(mrec).to_string().as_bytes().to_vec()) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        }

        Ok(())
    }

    fn add_cmdb(&mut self, mid: &str, crec: &MinionCmdbRecord) -> Result<(), SysinspectError> {
        let cmdb = self.get_tree(DB_CMDB)?;
        if let Err(err) = cmdb.insert(mid, json!(crec).to_string().as_bytes().to_vec()) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        }

        Ok(())
    }

    pub fn ensure_cmdb_registered(&mut self, mid: &str) -> Result<(), SysinspectError> {
        if self.get_cmdb(mid)?.is_none() {
            self.add_cmdb(mid, &MinionCmdbRecord::new(mid.to_string()))?;
        }

        Ok(())
    }

    pub fn upsert_cmdb_startup(&mut self, mid: &str, startup: MinionCmdbStartup) -> Result<(), SysinspectError> {
        let mut record = self.get_cmdb(mid)?.unwrap_or_else(|| MinionCmdbRecord::new(mid.to_string()));
        record.apply_startup(&startup);
        self.add_cmdb(mid, &record)
    }

    pub fn refresh_cmdb_observed(&mut self, mid: &str, traits: &HashMap<String, Value>) -> Result<(), SysinspectError> {
        let mut record = self.get_cmdb(mid)?.unwrap_or_else(|| MinionCmdbRecord::new(mid.to_string()));
        record.apply_observed_traits(traits);
        self.add_cmdb(mid, &record)
    }

    pub fn reconcile_cmdb(&mut self, mid: &str, max_age: std::time::Duration) -> Result<bool, SysinspectError> {
        let Some(mut record) = self.get_cmdb(mid)? else {
            return Ok(false);
        };
        if !record.is_stale(max_age) {
            return Ok(false);
        }
        let Some(minion) = self.get(mid)? else {
            return Ok(false);
        };

        record.apply_observed_traits(minion.get_traits());
        self.add_cmdb(mid, &record)?;

        Ok(true)
    }

    pub fn get_cmdb(&self, mid: &str) -> Result<Option<MinionCmdbRecord>, SysinspectError> {
        let cmdb = self.get_tree(DB_CMDB)?;
        let data = match cmdb.get(mid) {
            Ok(data) => data,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
        };

        if let Some(data) = data {
            return Ok(Some(match String::from_utf8(data.to_vec()) {
                Ok(data) => match serde_json::from_str::<MinionCmdbRecord>(&data) {
                    Ok(crec) => crec,
                    Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
                },
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            }));
        }

        Ok(None)
    }

    pub fn get_registered_ids(&self) -> Result<Vec<String>, SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        let mut ids: Vec<String> = Vec::new();

        for entry in minions.iter() {
            match entry {
                Ok((k_ent, _v_ent)) => {
                    let mid = String::from_utf8(k_ent.to_vec()).unwrap_or_default();
                    ids.push(mid);
                }
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Minion database seems corrupt: {err}"))),
            };
        }

        Ok(ids)
    }

    pub fn get(&self, mid: &str) -> Result<Option<MinionRecord>, SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        let data = match minions.get(mid) {
            Ok(data) => data,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
        };

        if let Some(data) = data {
            return Ok(Some(match String::from_utf8(data.to_vec()) {
                Ok(data) => match serde_json::from_str::<MinionRecord>(&data) {
                    Ok(mrec) => mrec,
                    Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
                },
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            }));
        }

        Ok(None)
    }

    pub fn remove(&mut self, mid: &str) -> Result<(), SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        let contains = match minions.contains_key(mid) {
            Ok(res) => res,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
        };

        if contains && let Err(err) = minions.remove(mid) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        };

        let cmdb = self.get_tree(DB_CMDB)?;
        let contains = match cmdb.contains_key(mid) {
            Ok(res) => res,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
        };

        if contains && let Err(err) = cmdb.remove(mid) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        };

        Ok(())
    }

    /// Get minion records by traits query
    /// Query is a string, matching hostnames as glob patterns.
    pub fn get_by_query(&self, query: &str) -> Result<Vec<MinionRecord>, SysinspectError> {
        let qmch = match Glob::new(query) {
            Ok(g) => g,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to compile query glob pattern: {err}"))),
        };

        let mut r: Vec<MinionRecord> = Vec::default();
        for e in self.get_tree(DB_MINIONS)?.iter() {
            match e {
                Ok((_, v)) => {
                    let mrec = serde_json::from_str::<MinionRecord>(&String::from_utf8(v.to_vec()).unwrap_or_default());
                    let mrec = match mrec {
                        Ok(mrec) => mrec,
                        Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to read minion record: {err}"))),
                    };

                    for tr in ["system.hostname", "system.hostname.fqdn"] {
                        if let Some(tr_val) = mrec.get_traits().get(tr)
                            && let Some(tr_val) = tr_val.as_str()
                            && qmch.compile_matcher().is_match(tr_val)
                        {
                            r.push(mrec.clone());
                            break;
                        }
                    }
                }
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Minion database seems corrupt: {err}"))),
            };
        }
        Ok(r)
    }

    /// Get minion records by hostname or IP address (fall-back)
    /// Receives hostname or IP address or any of these as glob pattern and returns matching minion records.
    pub fn get_by_hostname_or_ip(&mut self, hostname: &str) -> Result<Vec<MinionRecord>, SysinspectError> {
        let host_matcher = match Glob::new(hostname) {
            Ok(g) => g,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to compile hostname glob pattern: {err}"))),
        };

        let minions = self.get_tree(DB_MINIONS)?;
        let mut records: Vec<MinionRecord> = Vec::default();
        for entry in minions.iter() {
            match entry {
                Ok((_k_ent, v_ent)) => {
                    let mrec = serde_json::from_str::<MinionRecord>(&String::from_utf8(v_ent.to_vec()).unwrap_or_default());
                    let mrec = match mrec {
                        Ok(mrec) => mrec,
                        Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to read minion record: {err}"))),
                    };

                    for tr in ["system.hostname", "system.hostname.fqdn", "system.hostname.ip"] {
                        if let Some(tr_val) = mrec.get_traits().get(tr)
                            && let Some(tr_val) = tr_val.as_str()
                            && host_matcher.compile_matcher().is_match(tr_val)
                        {
                            records.push(mrec.clone());
                            break;
                        }
                    }
                }
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Minion database seems corrupt: {err}"))),
            };
        }
        Ok(records)
    }

    /// Select minions by trait criterias
    pub fn get_by_traits(&self, traits: HashMap<String, Value>) -> Result<Vec<MinionRecord>, SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        let mut mns: Vec<MinionRecord> = Vec::default();

        for entry in minions.iter() {
            match entry {
                Ok((_, v_ent)) => {
                    let mrec = serde_json::from_str::<MinionRecord>(&String::from_utf8(v_ent.to_vec()).unwrap_or_default());
                    let mrec = match mrec {
                        Ok(mrec) => mrec,
                        Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Unable to read minion record: {err}"))),
                    };

                    let mut matches = false;
                    for (kreq, vreq) in &traits {
                        if let Some(v) = mrec.get_traits().get(kreq) {
                            if vreq.eq(v) {
                                matches = true;
                            } else {
                                matches = false;
                                break;
                            }
                        } else {
                            matches = false;
                            break;
                        }
                    }

                    if matches {
                        mns.push(mrec);
                    }
                }
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Minion database seems corrupt: {err}"))),
            };
        }

        Ok(mns)
    }

    /// Get targeted minion IDs from a MinionTarget
    /// If `all` is true, return all matching minions regardless of their session status.
    pub async fn get_targeted_minions(&mut self, target: &MinionTarget, all: bool) -> Vec<String> {
        let session = Arc::clone(&SHARED_SESSION);
        let mut guard = session.lock().await;
        let mut ids = Vec::new();

        for mid in self.get_registered_ids().unwrap_or_default() {
            let Some(record) = self.get(&mid).ok().flatten() else {
                continue;
            };
            if Self::record_matches_target(&record, target) && (all || guard.alive(record.id())) {
                ids.push(record.id().to_string());
            }
        }

        ids.sort();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
#[path = "mreg_ut.rs"]
mod mreg_ut;
