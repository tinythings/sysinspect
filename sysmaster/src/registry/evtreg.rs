use libsysinspect::SysinspectError;
use sled::{Db, Tree};
use std::{fs, path::PathBuf};

const DB_EVENTS: &str = "events";

#[derive(Debug)]
pub struct EventsRegistry {
    conn: Db,
}

impl EventsRegistry {
    pub fn new(p: PathBuf) -> Result<EventsRegistry, SysinspectError> {
        if !p.exists() {
            fs::create_dir_all(&p)?;
        }

        Ok(EventsRegistry {
            conn: match sled::open(p) {
                Ok(db) => db,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            },
        })
    }

    /// Get a tree by a tree Id
    fn get_tree(&self, tid: &str) -> Result<Tree, SysinspectError> {
        match self.conn.open_tree(tid) {
            Ok(tree) => Ok(tree),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to open {tid} database: {err}"))),
        }
    }

    /// Add an event
    pub fn add(&mut self, mid: String, payload: String) -> Result<(), SysinspectError> {
        let events = self.get_tree(DB_EVENTS)?;
        match events.insert(mid, payload.as_bytes().to_vec()) {
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            _ => Ok(()),
        }
    }
}
