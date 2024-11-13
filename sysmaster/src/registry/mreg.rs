use super::rec::MinionRecord;
use libsysinspect::SysinspectError;
use serde_json::{json, Value};
use sled::Db;
use std::{collections::HashMap, fs, path::PathBuf};

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

    /// Add or update traits
    pub fn refresh(&mut self, mid: &str, traits: HashMap<String, Value>) -> Result<(), SysinspectError> {
        match self.conn.contains_key(mid) {
            Ok(exists) => {
                if exists {
                    if let Err(err) = self.conn.remove(mid) {
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
        if let Err(err) = self.conn.insert(mid, json!(mrec).to_string().as_bytes().to_vec()) {
            return Err(SysinspectError::MasterGeneralError(format!("{err}")));
        }

        Ok(())
    }

    pub fn get(&mut self, mid: &str) -> Result<Option<MinionRecord>, SysinspectError> {
        let data = match self.conn.get(mid) {
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
        let contains = match self.conn.contains_key(mid) {
            Ok(res) => res,
            Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
        };

        if contains {
            if let Err(err) = self.conn.remove(mid) {
                return Err(SysinspectError::MasterGeneralError(format!("{err}")));
            };
        }

        Ok(())
    }

    /// Select minions by trait criterias
    pub fn select(&self, traits: HashMap<String, Value>) -> Vec<String> {
        // XXX: Pretty much crude-dumb implementation which doesn't scale. But good enough for 0.x version. :-)
        vec![]
    }
}
