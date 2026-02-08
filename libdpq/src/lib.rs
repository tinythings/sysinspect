use libsysinspect::proto::MasterMessage;
use serde::{Deserialize, Serialize};
use sled::{Db, IVec};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkItem {
    MasterCommand(MasterMessage),
    // Next:
    // Kick(KickRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IncomingTask {
    id: u64,
    item: WorkItem,
    // optional metadata:
    // created_ts: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum QueueError {
    #[error("sled error: {0}")]
    Sled(#[from] sled::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// A simple durable work queue backed by sled embedded database.
/// Provides enqueue, dequeue, ack, and a runner loop with a notifier channel.
/// Designed for simplicity and durability, not high performance. Suitable for small to medium workloads.
///
/// Internally, it has 4 sled trees:
/// - `meta`: for storing metadata like next_id counter
/// - `pending`: keys are job ids that are pending (value is empty)
/// - `inflight`: keys are job ids that are currently being processed (value is empty)
/// - `jobs`: key is job id, value is serialized WorkItem payload
#[derive(Clone)]
pub struct WorkQueue {
    db: Arc<Db>,
    tx: mpsc::Sender<()>, // poke signal for tasks
}

impl WorkQueue {
    /// Open queue database at `path`, recover inflight -> pending.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, QueueError> {
        let db = sled::open(path)?;
        let db = Arc::new(db);

        // Trees
        db.open_tree("meta")?;
        db.open_tree("pending")?;
        db.open_tree("inflight")?;
        db.open_tree("jobs")?;

        let q = Self {
            db,
            tx: mpsc::channel::<()>(1).0, // placeholder, overwritten below
        };

        q.recover_inflight()?;
        let (tx, _rx) = mpsc::channel::<()>(1);

        Ok(Self { tx, ..q })
    }

    /// Hook for runner: get a receiver that wakes on enqueues.
    pub fn notifier(&self) -> mpsc::Receiver<()> {
        let (tx, rx) = mpsc::channel::<()>(1);
        // Replace internal tx with this tx by cloning queue (cheap) is awkward.
        // Instead: make notifier() only used once in start_runner() below.
        // So we don't expose this in final API; see start_runner().
        drop(tx);
        rx
    }

    fn meta(&self) -> sled::Tree {
        self.db.open_tree("meta").expect("meta tree")
    }
    fn pending(&self) -> sled::Tree {
        self.db.open_tree("pending").expect("pending tree")
    }
    fn inflight(&self) -> sled::Tree {
        self.db.open_tree("inflight").expect("inflight tree")
    }
    fn jobs(&self) -> sled::Tree {
        self.db.open_tree("jobs").expect("jobs tree")
    }

    fn next_id(&self) -> Result<u64, QueueError> {
        let meta = self.meta();
        let key = b"next_id";

        let next = meta
            .fetch_and_update(key, |old| {
                let mut v = old
                    .and_then(|x| {
                        let bytes = &x[..];
                        if bytes.len() != 8 {
                            log::warn!("Corrupt next_id in queue meta (len={}), resetting to 1", bytes.len());
                            return None;
                        }
                        let mut arr = [0u8; 8];
                        arr.copy_from_slice(bytes);
                        Some(u64::from_be_bytes(arr))
                    })
                    .unwrap_or(1);
                let out = v.to_be_bytes().to_vec();
                v += 1;
                Some(out)
            })?
            .map(|x| {
                let bytes = &x[..];
                if bytes.len() != 8 {
                    log::warn!("Corrupt next_id after update (len={}), resetting to 1", bytes.len());
                    return 1;
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(bytes);
                u64::from_be_bytes(arr)
            })
            .unwrap_or(1);

        Ok(next)
    }

    fn u64_key(id: u64) -> [u8; 8] {
        id.to_be_bytes()
    }

    fn store_job(&self, job: &IncomingTask) -> Result<(), QueueError> {
        let jobs = self.jobs();
        let key = Self::u64_key(job.id);
        let val = serde_json::to_vec(job)?;
        jobs.insert(key, val)?;
        Ok(())
    }

    fn load_job(&self, id: u64) -> Result<Option<IncomingTask>, QueueError> {
        let jobs = self.jobs();
        let key = Self::u64_key(id);
        let Some(v) = jobs.get(key)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice::<IncomingTask>(&v)?))
    }

    /// Add a new job to the queue, returns job id.
    pub fn enqueue(&self, item: WorkItem) -> Result<u64, QueueError> {
        let id = self.next_id()?;
        let job = IncomingTask { id, item };

        self.store_job(&job)?;
        self.pending().insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        self.db.flush()?;

        let _ = self.tx.try_send(());

        Ok(id)
    }

    /// Try to take one pending job and mark inflight.
    pub fn try_dequeue(&self) -> Result<Option<(u64, WorkItem)>, QueueError> {
        let pending = self.pending();
        let mut it = pending.iter();
        let Some(Ok((k, _v))) = it.next() else {
            return Ok(None);
        };

        let id = u64::from_be_bytes(k.as_ref().try_into().unwrap());

        // Atomic-ish move: remove from pending, add to inflight.
        // If crash happens after remove but before inflight insert, job is still in jobs tree,
        // but not referenced. To be safer, insert inflight first then remove pending.
        self.inflight().insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        pending.remove(Self::u64_key(id))?;

        self.db.flush()?;

        let Some(job) = self.load_job(id)? else {
            // Corruption or manual tampering
            self.inflight().remove(Self::u64_key(id))?;
            self.db.flush()?;
            return Ok(None);
        };

        Ok(Some((id, job.item)))
    }

    /// Mark job done: remove inflight + delete payload.
    pub fn ack_done(&self, id: u64) -> Result<(), QueueError> {
        self.inflight().remove(Self::u64_key(id))?;
        self.jobs().remove(Self::u64_key(id))?;
        self.db.flush()?;
        Ok(())
    }

    /// Recovery on startup (e.g. someone killed the process, rebooted the minion etc)
    pub fn recover_inflight(&self) -> Result<(), QueueError> {
        let inflight = self.inflight();
        let pending = self.pending();
        let keys: Vec<[u8; 8]> = inflight
            .iter()
            .filter_map(|res| res.ok())
            .filter_map(|(k, _)| {
                let arr: Option<[u8; 8]> = k.as_ref().try_into().ok();
                arr
            })
            .collect();

        if !keys.is_empty() {
            for k in keys {
                pending.insert(k, IVec::from(&b""[..]))?;
                inflight.remove(k)?;
            }
            self.db.flush()?;
        }
        Ok(())
    }

    /// Start a runner loop
    pub fn start<F, Fut>(&self, mut exec: F)
    where
        F: FnMut(u64, WorkItem) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // Notifier channel coalesces wakeups. Runner also wakes periodically
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let mut q = self.clone();
        q.tx = tx;

        tokio::spawn(async move {
            loop {
                // Try run as much as possible
                loop {
                    match q.try_dequeue() {
                        Ok(Some((id, item))) => {
                            exec(id, item).await;
                        }
                        Ok(None) => break,
                        Err(e) => {
                            log::error!("queue dequeue error: {e}");
                            break;
                        }
                    }
                }

                tokio::select! {
                    _ = rx.recv() => {},
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {},
                }
            }
        });
    }
}
