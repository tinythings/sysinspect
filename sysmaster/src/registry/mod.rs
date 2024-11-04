/*
Minion registry. It contains minion tasks, traits, location and other data
 */

pub mod rec;

use libsysinspect::SysinspectError;
use rec::MinionRecord;
use serde_json::json;
use sled::Db;

static CFG_DEFAULT_ROOT: &str = "/etc/sysinspect";
static CFG_DB: &str = "registry";
static CFG_MINION_KEYS: &str = "minion-keys";

pub struct MinionRegistry {
    conn: Db,
}

impl MinionRegistry {
    pub fn new(pth: &str) -> Result<MinionRegistry, SysinspectError> {
        Ok(MinionRegistry {
            conn: match sled::open(pth) {
                Ok(db) => db,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            },
        })
    }

    pub fn add(&mut self, mid: &str, mrec: MinionRecord) -> Result<(), SysinspectError> {
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
}
