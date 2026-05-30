use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InboundCommandState {
    Accepted,
    Running,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InboundCommandRecord {
    replay_key: String,
    cycle_id: String,
    state: InboundCommandState,
    updated_at_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundCommandClaim {
    AcceptedNew,
    Duplicate(InboundCommandState),
}

#[derive(Clone, Debug)]
pub struct InboundCommandLedger {
    db: Arc<Db>,
    entries: Tree,
}

impl InboundCommandLedger {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SysinspectError> {
        let db = Arc::new(sled::open(path)?);
        let entries = db.open_tree("entries")?;
        Ok(Self { db, entries })
    }

    fn now_ms() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    }

    pub fn claim(&self, replay_key: &str, cycle_id: &str) -> Result<InboundCommandClaim, SysinspectError> {
        if let Some(raw) = self.entries.get(replay_key)? {
            let record: InboundCommandRecord = serde_json::from_slice(&raw)?;
            return Ok(InboundCommandClaim::Duplicate(record.state));
        }

        let record = InboundCommandRecord {
            replay_key: replay_key.to_string(),
            cycle_id: cycle_id.to_string(),
            state: InboundCommandState::Accepted,
            updated_at_ms: Self::now_ms(),
        };
        self.entries.insert(replay_key, serde_json::to_vec(&record)?)?;
        self.db.flush()?;
        Ok(InboundCommandClaim::AcceptedNew)
    }

    pub fn set_state(&self, replay_key: &str, state: InboundCommandState) -> Result<bool, SysinspectError> {
        let Some(raw) = self.entries.get(replay_key)? else {
            return Ok(false);
        };
        let mut record: InboundCommandRecord = serde_json::from_slice(&raw)?;
        record.state = state;
        record.updated_at_ms = Self::now_ms();
        self.entries.insert(replay_key, serde_json::to_vec(&record)?)?;
        self.db.flush()?;
        Ok(true)
    }

    pub fn state(&self, replay_key: &str) -> Result<Option<InboundCommandState>, SysinspectError> {
        let Some(raw) = self.entries.get(replay_key)? else {
            return Ok(None);
        };
        let record: InboundCommandRecord = serde_json::from_slice(&raw)?;
        Ok(Some(record.state))
    }

    pub fn remove(&self, replay_key: &str) -> Result<bool, SysinspectError> {
        let removed = self.entries.remove(replay_key)?.is_some();
        if removed {
            self.db.flush()?;
        }
        Ok(removed)
    }
}
