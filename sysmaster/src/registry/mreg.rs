use super::rec::MinionRecord;
use libsysinspect::SysinspectError;
use serde_json::{json, Value};
use sled::{Db, Tree};
use std::{collections::HashMap, fs, path::PathBuf};

const DB_MINIONS: &str = "minions";

#[derive(Debug)]
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

    pub fn get(&mut self, mid: &str) -> Result<Option<MinionRecord>, SysinspectError> {
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

        if contains {
            if let Err(err) = minions.remove(mid) {
                return Err(SysinspectError::MasterGeneralError(format!("{err}")));
            };
        }

        Ok(())
    }

    /// Select minions by trait criterias
    pub fn select(&self, traits: HashMap<String, Value>) -> Result<Vec<String>, SysinspectError> {
        let minions = self.get_tree(DB_MINIONS)?;
        let mut mns: Vec<String> = Vec::default();

        // XXX: Pretty much crude-dumb implementation which doesn't scale. But good enough for 0.x version. :-)
        for entry in minions.iter() {
            match entry {
                Ok((k_ent, v_ent)) => {
                    let mid = String::from_utf8(k_ent.to_vec()).unwrap_or_default();
                    let mrec = serde_json::from_str::<MinionRecord>(&String::from_utf8(v_ent.to_vec()).unwrap_or_default());
                    let mrec = match mrec {
                        Ok(mrec) => mrec,
                        Err(err) => {
                            return Err(SysinspectError::MasterGeneralError(format!("Unable to read minion record: {err}")))
                        }
                    };

                    let mut matches = false;
                    for (kreq, vreq) in &traits {
                        if let Some(v) = mrec.get_traits().get(kreq) {
                            if vreq.eq(v) {
                                matches = true;
                            } else {
                                matches = true;
                                break;
                            }
                        } else {
                            matches = false;
                            break;
                        }
                    }

                    if matches {
                        mns.push(mid);
                    }
                }
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Minion database seems corrupt: {err}"))),
            };
        }

        Ok(mns)
    }
}
