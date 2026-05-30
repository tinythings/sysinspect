use libcommon::SysinspectError;
use libsysproto::MasterMessage;
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
#[path = "cmdq_ut.rs"]
mod cmdq_ut;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum MasterCommandState {
    #[default]
    Pending,
    Replayed,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMasterCommand {
    id: u64,
    replay_key: String,
    minion_id: String,
    message: MasterMessage,
    state: MasterCommandState,
    enqueued_at_ms: u128,
}

impl QueuedMasterCommand {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn replay_key(&self) -> &str {
        &self.replay_key
    }

    pub fn message(&self) -> &MasterMessage {
        &self.message
    }

    pub fn state(&self) -> MasterCommandState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MasterCommandQueueStats {
    pub pending_commands: usize,
    pub replayed_commands: usize,
    pub queued_minions: usize,
}

impl MasterCommandQueueStats {
    pub fn format(self) -> String {
        format!("{}/{}/{} pending/replayed/minions", self.pending_commands, self.replayed_commands, self.queued_minions)
    }
}

#[derive(Clone, Debug)]
pub struct MasterCommandQueue {
    db: Arc<Db>,
    meta: Tree,
    entries: Tree,
}

impl MasterCommandQueue {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SysinspectError> {
        let db = Arc::new(sled::open(path)?);
        let meta = db.open_tree("meta")?;
        let entries = db.open_tree("entries")?;
        let queue = Self { db, meta, entries };
        let stats = queue.stats()?;
        if stats.pending_commands > 0 || stats.replayed_commands > 0 {
            log::warn!("Recovered master outbound command backlog: {}", stats.format());
        }
        Ok(queue)
    }

    fn u64_key(id: u64) -> [u8; 8] {
        id.to_be_bytes()
    }

    fn bytes_to_u64(bytes: &[u8]) -> Option<u64> {
        if bytes.len() != 8 {
            return None;
        }
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Some(u64::from_be_bytes(arr))
    }

    fn next_id(&self) -> Result<u64, SysinspectError> {
        let key = b"next_id";
        let old = self.meta.fetch_and_update(key, |old| {
            let current = old.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or(1);
            Some(current.saturating_add(1).to_be_bytes().to_vec())
        })?;
        Ok(old.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or(1))
    }

    fn now_ms() -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
    }

    pub fn replay_key(minion_id: &str, cycle_id: &str) -> String {
        format!("mcmd|{minion_id}|{cycle_id}")
    }

    pub fn enqueue(&self, minion_id: &str, message: &MasterMessage) -> Result<u64, SysinspectError> {
        let id = self.next_id()?;
        let entry = QueuedMasterCommand {
            id,
            replay_key: Self::replay_key(minion_id, message.cycle()),
            minion_id: minion_id.to_string(),
            message: message.clone(),
            state: MasterCommandState::Pending,
            enqueued_at_ms: Self::now_ms(),
        };
        self.entries.insert(Self::u64_key(id), serde_json::to_vec(&entry)?)?;
        self.db.flush()?;
        log::debug!("Queued durable master command {} for {} cycle {}", id, minion_id, message.cycle());
        Ok(id)
    }

    fn load_entry(&self, id: u64) -> Result<Option<QueuedMasterCommand>, SysinspectError> {
        let Some(raw) = self.entries.get(Self::u64_key(id))? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(&raw)?))
    }

    fn save_entry(&self, entry: &QueuedMasterCommand) -> Result<(), SysinspectError> {
        self.entries.insert(Self::u64_key(entry.id), serde_json::to_vec(entry)?)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn set_state(&self, id: u64, state: MasterCommandState) -> Result<bool, SysinspectError> {
        let Some(mut entry) = self.load_entry(id)? else {
            return Ok(false);
        };
        entry.state = state;
        self.save_entry(&entry)?;
        Ok(true)
    }

    pub fn remove(&self, id: u64) -> Result<bool, SysinspectError> {
        let removed = self.entries.remove(Self::u64_key(id))?.is_some();
        if removed {
            self.db.flush()?;
        }
        Ok(removed)
    }

    pub fn pending_for_minion(&self, minion_id: &str) -> Result<Vec<QueuedMasterCommand>, SysinspectError> {
        let mut out = Vec::new();
        for item in self.entries.iter() {
            let (_k, v) = item?;
            let entry: QueuedMasterCommand = serde_json::from_slice(&v)?;
            if entry.minion_id == minion_id && entry.state != MasterCommandState::Cleared {
                out.push(entry);
            }
        }
        out.sort_by_key(|entry| entry.id);
        Ok(out)
    }

    pub fn stats(&self) -> Result<MasterCommandQueueStats, SysinspectError> {
        let mut stats = MasterCommandQueueStats::default();
        let mut minions = BTreeSet::new();
        for item in self.entries.iter() {
            let (_k, v) = item?;
            let entry: QueuedMasterCommand = serde_json::from_slice(&v)?;
            match entry.state {
                MasterCommandState::Pending => {
                    stats.pending_commands += 1;
                    minions.insert(entry.minion_id);
                }
                MasterCommandState::Replayed => {
                    stats.replayed_commands += 1;
                    minions.insert(entry.minion_id);
                }
                MasterCommandState::Cleared => {}
            }
        }
        stats.queued_minions = minions.len();
        Ok(stats)
    }
}
