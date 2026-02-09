use crate::master::SHARED_SESSION;

use super::rec::MinionRecord;
use globset::Glob;
use libcommon::SysinspectError;
use libsysproto::MinionTarget;
use serde_json::{Value, json};
use sled::{Db, Tree};
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

const DB_MINIONS: &str = "minions";

#[derive(Debug, Clone)]
pub struct MinionRegistry {
    conn: Db,
}

impl MinionRegistry {
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
    pub fn refresh(&mut self, mid: &str, traits: HashMap<String, Value>) -> Result<(), SysinspectError> {
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

        self.add(mid, MinionRecord::new(mid.to_string(), traits))?;

        Ok(())
    }

    fn add(&mut self, mid: &str, mrec: MinionRecord) -> Result<(), SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        if let Err(err) = minions.insert(mid, json!(mrec).to_string().as_bytes().to_vec()) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        }

        Ok(())
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
        // Direct ID specified
        let session = Arc::clone(&SHARED_SESSION);

        if !target.id().is_empty() {
            return vec![target.id().to_string()];
        }

        // Hostnames are specified
        if !target.hostnames().is_empty() {
            let mut ids: Vec<String> = Vec::new();
            for hn in target.hostnames() {
                match self.get_by_hostname_or_ip(&hn) {
                    Ok(mrecs) => {
                        for mrec in mrecs {
                            if all || session.lock().await.alive(mrec.id()) {
                                ids.push(mrec.id().to_string());
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Unable to get minion by hostname {}: {}", hn, err);
                    }
                }
            }
            return ids;
        }

        vec![]
    }
}
