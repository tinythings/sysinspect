use libsysinspect::{SysinspectError, proto::MasterMessage};
use serde::{Deserialize, Serialize};
use sled::{Db, IVec, Tree};
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
}

#[derive(Clone)]
pub struct DiskPersistentQueue {
    db: Arc<Db>,
    meta: Tree,
    pending: Tree,
    inflight: Tree,
    jobs: Tree,

    tx: mpsc::Sender<()>,
}

impl DiskPersistentQueue {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SysinspectError> {
        let db = Arc::new(sled::open(path)?);

        let meta = db.open_tree("meta")?;
        let pending = db.open_tree("pending")?;
        let inflight = db.open_tree("inflight")?;
        let jobs = db.open_tree("jobs")?;

        let q = Self {
            db,
            meta,
            pending,
            inflight,
            jobs,
            tx: mpsc::channel::<()>(1).0, // placeholder
        };

        q.recover_inflight()?;

        let (tx, _rx) = mpsc::channel::<()>(1);
        Ok(Self { tx, ..q })
    }

    #[inline]
    fn u64_key(id: u64) -> [u8; 8] {
        id.to_be_bytes()
    }

    #[inline]
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

        let updated = self.meta.fetch_and_update(key, |old| {
            let mut v = old.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or(1);

            let out = v.to_be_bytes().to_vec();
            v = v.saturating_add(1);
            Some(out)
        })?;

        let next = updated.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or_else(|| {
            log::warn!("Corrupt next_id in queue meta, resetting to 1");
            1
        });

        Ok(next)
    }

    fn store_job(&self, job: &IncomingTask) -> Result<(), SysinspectError> {
        let key = Self::u64_key(job.id);
        let val = serde_json::to_vec(job)?;
        self.jobs.insert(key, val)?;
        Ok(())
    }

    fn load_job(&self, id: u64) -> Result<Option<IncomingTask>, SysinspectError> {
        let key = Self::u64_key(id);
        let Some(v) = self.jobs.get(key)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice::<IncomingTask>(&v)?))
    }

    pub fn enqueue(&self, item: WorkItem) -> Result<u64, SysinspectError> {
        let id = self.next_id()?;
        let job = IncomingTask { id, item };

        self.store_job(&job)?;
        self.pending.insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        self.db.flush()?;

        let _ = self.tx.try_send(());
        Ok(id)
    }

    pub fn try_dequeue(&self) -> Result<Option<(u64, WorkItem)>, SysinspectError> {
        let Some(Ok((k, _v))) = self.pending.iter().next() else {
            return Ok(None);
        };

        let id = match Self::bytes_to_u64(k.as_ref()) {
            Some(id) => id,
            None => {
                log::warn!("Corrupt pending key len={}, removing it", k.len());
                self.pending.remove(k)?;
                self.db.flush()?;
                return Ok(None);
            }
        };

        // move pending -> inflight
        self.inflight.insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        self.pending.remove(Self::u64_key(id))?;
        self.db.flush()?;

        let Some(job) = self.load_job(id)? else {
            log::warn!("Job payload missing for id={id}, cleaning inflight marker");
            self.inflight.remove(Self::u64_key(id))?;
            self.db.flush()?;
            return Ok(None);
        };

        Ok(Some((id, job.item)))
    }

    pub fn ack_done(&self, id: u64) -> Result<(), SysinspectError> {
        self.inflight.remove(Self::u64_key(id))?;
        self.jobs.remove(Self::u64_key(id))?;
        self.db.flush()?;
        Ok(())
    }

    pub fn recover_inflight(&self) -> Result<(), SysinspectError> {
        let keys: Vec<[u8; 8]> = self
            .inflight
            .iter()
            .filter_map(|res| res.ok())
            .filter_map(|(k, _)| {
                let bytes = k.as_ref();
                if bytes.len() != 8 {
                    log::warn!("Corrupt inflight key len={}, skipping", bytes.len());
                    return None;
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(bytes);
                Some(arr)
            })
            .collect();

        for k in &keys {
            self.pending.insert(k, IVec::from(&b""[..]))?;
            self.inflight.remove(k)?;
        }

        if !keys.is_empty() {
            self.db.flush()?;
        }

        Ok(())
    }

    pub fn start<F, Fut>(&self, mut exec: F)
    where
        F: FnMut(u64, WorkItem) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let mut q = self.clone();
        q.tx = tx;

        tokio::spawn(async move {
            loop {
                loop {
                    match q.try_dequeue() {
                        Ok(Some((id, item))) => exec(id, item).await,
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
