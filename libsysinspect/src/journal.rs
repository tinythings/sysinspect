//! Cycle-keyed persistent journal with ACK-based eviction.
//!
//! Payloads are grouped by `cycle_id`.  When the consumer acknowledges a
//! cycle all its entries are freed.  A byte budget caps un-acked data;
//! the oldest cycle is evicted when the budget is exceeded.

use libcommon::SysinspectError;
use sled::{Db, Tree};
use std::{path::Path, sync::Arc};

pub type CycleEntry = (u64, Vec<u8>);
pub type CycleEntries = Vec<(String, Vec<CycleEntry>)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JournalStats {
    pub pending_cycles: usize,
    pub pending_entries: usize,
    pub pending_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct Journal {
    db: Arc<Db>,
    pending: Tree,   // key = cycle_id:seq, val = payload
    seq: Tree,       // key = cycle_id, val = next_seq (u64 BE)
    acked: Tree,     // key = cycle_id, val = "" (marker, entries already deleted)
    completed: Tree, // key = cycle_id, val = "" (local execution finished, delivery may still be pending)
    max_bytes: u64,
}

impl Journal {
    pub fn open<P: AsRef<Path>>(path: P, max_bytes: u64) -> Result<Self, SysinspectError> {
        let config = sled::Config::new().flush_every_ms(Some(0)).path(&path);
        let db = Arc::new(config.open()?);
        Ok(Self {
            pending: db.open_tree("pending")?,
            seq: db.open_tree("seq")?,
            acked: db.open_tree("acked")?,
            completed: db.open_tree("completed")?,
            db,
            max_bytes,
        })
    }

    // ---- helpers ----

    fn pending_key(cycle_id: &str, seq: u64) -> Vec<u8> {
        let mut k = Vec::with_capacity(cycle_id.len() + 1 + 8);
        k.extend_from_slice(cycle_id.as_bytes());
        k.push(b':');
        k.extend_from_slice(&seq.to_be_bytes());
        k
    }

    fn cycle_prefix(cycle_id: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(cycle_id.len() + 1);
        k.extend_from_slice(cycle_id.as_bytes());
        k.push(b':');
        k
    }

    fn next_seq(&self, cycle_id: &str) -> Result<u64, SysinspectError> {
        let raw = self.seq.fetch_and_update(cycle_id.as_bytes(), |old| {
            let cur = old.and_then(|v| if v.len() >= 8 { Some(u64::from_be_bytes(v[..8].try_into().unwrap())) } else { None }).unwrap_or(0);
            Some(cur.wrapping_add(1).to_be_bytes().to_vec())
        })?;
        let assigned = raw.and_then(|v| if v.len() >= 8 { Some(u64::from_be_bytes(v[..8].try_into().unwrap())) } else { None }).unwrap_or(0);
        Ok(assigned)
    }

    fn total_key() -> &'static [u8] {
        b"__total_bytes"
    }

    fn add_total(&self, delta: i64) -> Result<u64, SysinspectError> {
        let raw = self.pending.fetch_and_update(Self::total_key(), |old| {
            let cur = old.and_then(|v| if v.len() >= 8 { Some(i64::from_be_bytes(v[..8].try_into().unwrap())) } else { None }).unwrap_or(0);
            Some((cur + delta).max(0).to_be_bytes().to_vec())
        })?;
        Ok(raw.and_then(|v| if v.len() >= 8 { Some(i64::from_be_bytes(v[..8].try_into().unwrap()) as u64) } else { None }).unwrap_or(0))
    }

    fn _total_bytes(&self) -> u64 {
        self.pending
            .get(Self::total_key())
            .ok()
            .flatten()
            .and_then(|v| if v.len() >= 8 { Some(i64::from_be_bytes(v[..8].try_into().unwrap()) as u64) } else { None })
            .unwrap_or(0)
    }

    fn evict_if_needed(&self) {
        if self.max_bytes == 0 {
            return;
        }
        while self._total_bytes() > self.max_bytes {
            let Some(oldest) = self.oldest_unacked_cycle() else {
                break;
            };
            log::warn!("Journal exceeded byte budget ({} > {}); evicting oldest unacked cycle {}", self._total_bytes(), self.max_bytes, oldest);
            self.ack_cycle_inner(&oldest);
        }
    }

    fn oldest_unacked_cycle(&self) -> Option<String> {
        for item in self.pending.iter() {
            let (k, _) = item.ok()?;
            let key_bytes = k.as_ref();
            if key_bytes.starts_with(b"__") {
                continue;
            }
            let colon = key_bytes.iter().rposition(|&b| b == b':')?;
            return Some(String::from_utf8_lossy(&key_bytes[..colon]).to_string());
        }
        None
    }

    fn ack_cycle_inner(&self, cycle_id: &str) -> usize {
        let prefix = Self::cycle_prefix(cycle_id);
        let keys: Vec<sled::IVec> = self.pending.scan_prefix(&prefix).filter_map(|r| r.ok().map(|(k, _)| k)).collect();
        let mut count = 0usize;
        for k in &keys {
            if let Ok(Some(v)) = self.pending.get(k) {
                let _ = self.add_total(-(v.len() as i64));
            }
            if self.pending.remove(k).is_ok() {
                count += 1;
            }
        }
        let _ = self.acked.insert(cycle_id.as_bytes(), &[][..]);
        let _ = self.completed.remove(cycle_id.as_bytes());
        let _ = self.db.flush();
        count
    }

    // ---- public API ----

    /// Append a payload under a cycle.  Returns the per-cycle sequence number.
    pub fn append(&self, cycle_id: &str, payload: &[u8]) -> Result<u64, SysinspectError> {
        if self.acked.contains_key(cycle_id.as_bytes())? {
            self.acked.remove(cycle_id.as_bytes())?;
        }
        let seq = self.next_seq(cycle_id)?;
        self.pending.insert(Self::pending_key(cycle_id, seq), payload)?;
        self.add_total(payload.len() as i64)?;
        self.db.flush()?;
        if self.max_bytes > 0 {
            self.evict_if_needed();
        }
        Ok(seq)
    }

    /// Acknowledge a cycle — all its entries are deleted.
    /// Idempotent: calling twice on the same cycle is safe.
    /// Returns the number of entries freed.
    pub fn ack_cycle(&self, cycle_id: &str) -> Result<usize, SysinspectError> {
        if self.acked.contains_key(cycle_id.as_bytes())? {
            return Ok(0);
        }
        Ok(self.ack_cycle_inner(cycle_id))
    }

    /// Mark a cycle as locally completed. Delivery may still be pending until master `CycleAck` arrives.
    pub fn mark_cycle_locally_complete(&self, cycle_id: &str) -> Result<(), SysinspectError> {
        self.completed.insert(cycle_id.as_bytes(), &[][..])?;
        self.db.flush()?;
        Ok(())
    }

    /// Return whether the cycle has already finished local execution and only delivery recovery may remain.
    pub fn is_cycle_locally_complete(&self, cycle_id: &str) -> Result<bool, SysinspectError> {
        self.completed.contains_key(cycle_id.as_bytes()).map_err(Into::into)
    }

    /// Return current backlog counters for operator visibility.
    pub fn stats(&self) -> Result<JournalStats, SysinspectError> {
        let mut stats = JournalStats { pending_bytes: self._total_bytes(), ..Default::default() };
        let mut last_cycle: Option<String> = None;
        for item in self.pending.iter() {
            let (k, _v) = item?;
            let key_bytes = k.as_ref();
            if key_bytes.starts_with(b"__") {
                continue;
            }
            let Some(colon) = key_bytes.iter().rposition(|&b| b == b':') else {
                continue;
            };
            let cycle_id = String::from_utf8_lossy(&key_bytes[..colon]).to_string();
            stats.pending_entries += 1;
            if last_cycle.as_deref() != Some(cycle_id.as_str()) {
                stats.pending_cycles += 1;
                last_cycle = Some(cycle_id);
            }
        }
        Ok(stats)
    }

    /// Return all un-acked cycles with their entries in insertion order.
    pub fn pending(&self) -> Result<CycleEntries, SysinspectError> {
        let mut cycles: CycleEntries = Vec::new();
        for item in self.pending.iter() {
            let (k, v) = item?;
            let key_bytes = k.as_ref();
            if key_bytes.starts_with(b"__") {
                continue;
            }
            let colon = match key_bytes.iter().rposition(|&b| b == b':') {
                Some(pos) => pos,
                None => continue,
            };
            let cycle_id = String::from_utf8_lossy(&key_bytes[..colon]).to_string();
            let seq = if key_bytes.len() >= colon + 9 {
                u64::from_be_bytes(key_bytes[colon + 1..colon + 9].try_into().unwrap())
            } else {
                continue;
            };
            if let Some(last) = cycles.last_mut()
                && last.0 == cycle_id
            {
                last.1.push((seq, v.to_vec()));
            } else {
                cycles.push((cycle_id, vec![(seq, v.to_vec())]));
            }
        }
        Ok(cycles)
    }
}
