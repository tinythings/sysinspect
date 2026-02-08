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

/// Disk-backed persistent queue for background job processing.
/// This is a simple implementation that uses sled as the underlying storage engine.
/// It supports adding jobs, fetching jobs for processing, acknowledging completed jobs, and retrying failed jobs.
/// It also has a recovery mechanism to move inflight jobs back to pending on startup, in case the process was killed while processing some jobs.
/// Note: this implementation does not have features like delayed retries, dead-letter queues, or visibility timeouts, but it can be extended with those if needed.
/// Example usage:
/// ```
/// let q = DiskPersistentQueue::open("/path/to/queue")?;
/// let item = WorkItem::MasterCommand(...);
/// let job_id = q.add(item)?;
/// println!("Enqueued job with ID: {job_id}");
/// // Start a runner to process jobs:
/// q.start_runner(|job_id, item| async move {
///     // Process the item...
///     // Then ack or nack:
///     if success {
///         q.ack(job_id)?;
///     } else {
///         q.nack(job_id)?;
///     }
/// });
/// ```
#[derive(Clone, Debug)]
pub struct DiskPersistentQueue {
    db: Arc<Db>,
    meta: Tree,
    pending: Tree,
    inflight: Tree,
    jobs: Tree,

    tx: mpsc::Sender<()>,
}

impl DiskPersistentQueue {
    /// Open or create a new disk-backed queue at the specified path.
    /// This will create the necessary sled database and trees if they don't exist.
    /// It will also perform recovery by moving any inflight jobs back to pending, so they can be retried.
    /// Returns a DiskPersistentQueue instance on success, or an error if the database cannot be opened or initialized.
    /// Example usage:
    /// ```
    /// let q = DiskPersistentQueue::open("/path/to/queue")?;
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SysinspectError> {
        let db = Arc::new(sled::open(path)?);

        let meta = db.open_tree("meta")?;
        let pending = db.open_tree("pending")?;
        let inflight = db.open_tree("inflight")?;
        let jobs = db.open_tree("jobs")?;

        let (tx, _rx) = mpsc::channel::<()>(1);
        let q = Self { db, meta, pending, inflight, jobs, tx };
        q.recover()?;

        // Wake up any runners waiting for jobs
        let _ = q.tx.try_send(());

        Ok(q)
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

    /// Get the next job ID and increment the counter atomically.
    ///
    /// This method uses the `fetch_and_update` operation on the `meta` tree to ensure that each
    /// job gets a unique ID even in concurrent scenarios. The job ID is stored as a big-endian u64
    /// in the `meta` tree under the key "next_id". If the key does not exist, it starts from 1.
    /// Returns the next job ID on success, or an error if the operation fails.
    ///
    /// Example usage:
    /// ```
    /// let job_id = q.next_id()?;
    /// println!("Next job ID: {job_id}");
    /// ```

    fn next_id(&self) -> Result<u64, SysinspectError> {
        let key = b"next_id";
        let old = self.meta.fetch_and_update(key, |old| {
            let current = old.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or(1);

            let next = current.saturating_add(1);
            Some(next.to_be_bytes().to_vec())
        })?;

        let assigned = old.as_ref().and_then(|x| Self::bytes_to_u64(&x[..])).unwrap_or(1);

        Ok(assigned)
    }

    /// Store the task payload in the jobs tree, keyed by the job ID.
    /// This is separate from the pending/inflight markers to allow for more
    /// flexible recovery and potential future features like delayed retries or dead-letter queues.
    ///
    /// Returns an error if the storage process fails, or Ok(()) if it succeeds.
    ///
    /// Example usage:
    /// ```
    /// let item = WorkItem::MasterCommand(...);
    /// q.store_task(job_id, &item).unwrap();
    /// ```
    fn store_task(&self, id: u64, item: &WorkItem) -> Result<(), SysinspectError> {
        let key = Self::u64_key(id);
        let val = serde_json::to_vec(item)?;
        self.jobs.insert(key, val)?;
        Ok(())
    }

    /// Load the task payload from the jobs tree using the job ID.
    /// Returns Ok(Some(WorkItem)) if the task is found and deserialized successfully,
    /// Ok(None) if the task is not found, or Err if there is an error during the process.
    /// Example usage:
    /// ```
    /// match q.load_task(job_id) {
    ///     Ok(Some(item)) => {
    ///         // Process the item...
    ///     }
    ///     Ok(None) => {
    ///         println!("Task with ID {job_id} not found");
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Error loading task with ID {job_id}: {e}");
    ///     }
    /// }
    /// ```
    fn load_task(&self, id: u64) -> Result<Option<WorkItem>, SysinspectError> {
        let key = Self::u64_key(id);
        let Some(v) = self.jobs.get(key)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice::<WorkItem>(&v)?))
    }

    /// Add a new job to the queue. Returns the assigned job ID on success.
    /// Example usage:
    /// ```
    /// let item = WorkItem::MasterCommand(...);
    /// let job_id = q.add(item).unwrap();
    /// println!("Enqueued job with ID: {job_id}");
    /// ```
    pub fn add(&self, item: WorkItem) -> Result<u64, SysinspectError> {
        let id = self.next_id()?;
        self.store_task(id, &item)?;
        self.pending.insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        self.db.flush()?;

        let _ = self.tx.try_send(());
        Ok(id)
    }

    /// Fetch/dequeue a job for processing. This moves the job from pending to inflight, so it won't be picked up by other runners.
    /// Returns Ok(Some((id, item))) if a job is available, Ok(None) if no jobs are pending, or Err if there is an error during the process.
    /// The caller should call ack(id) when the job is done, or nack(id) if it fails and should be retried.
    /// Example usage:
    /// ```
    /// match q.fetch() {
    ///     Ok(Some((id, item))) => {
    ///         // Process the item...
    ///         // Then ack or nack:
    ///         if success {
    ///             q.ack(id).unwrap();
    ///         } else {
    ///             q.nack(id).unwrap();
    ///         }
    ///     }
    ///     Ok(None) => {
    ///         println!("No jobs pending");
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Error during dequeue: {e}");
    ///     }
    /// }
    pub fn fetch(&self) -> Result<Option<(u64, WorkItem)>, SysinspectError> {
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

        let Some(item) = self.load_task(id)? else {
            log::error!("Job payload missing for id={id}; dropping markers (queue corruption or race)");
            self.inflight.remove(Self::u64_key(id))?;
            self.pending.remove(Self::u64_key(id))?; // just in case
            self.db.flush()?;
            return Ok(None);
        };
        Ok(Some((id, item)))
    }

    /// Ack means the job is done and can be removed from the queue.
    ///
    /// Returns an error if the ack process fails, or Ok(()) if it succeeds.
    /// Example usage:
    /// ```
    /// if let Err(e) = q.ack(job_id) {
    ///     eprintln!("Ack failed for {job_id}: {e}");
    /// }
    pub fn ack(&self, id: u64) -> Result<(), SysinspectError> {
        self.inflight.remove(Self::u64_key(id))?;
        self.jobs.remove(Self::u64_key(id))?;
        self.db.flush()?;
        Ok(())
    }

    /// Nack means the job is not done and should be retried later.
    ///
    /// Moves the job back to pending, so it can be picked up again by a runner.
    /// This does not implement any retry limits or backoff, so it may cause hot
    /// loops if a job keeps failing. Use with caution.
    ///
    /// Returns an error if the nack process fails, or Ok(()) if it succeeds.
    ///
    /// Example usage:
    /// ```
    /// if let Err(e) = q.nack(job_id) {
    ///     eprintln!("Nack failed for {job_id}: {e}");
    /// }
    pub fn nack(&self, id: u64) -> Result<(), SysinspectError> {
        self.pending.insert(Self::u64_key(id), IVec::from(&b""[..]))?;
        self.inflight.remove(Self::u64_key(id))?;
        self.db.flush()?;
        let _ = self.tx.try_send(());
        Ok(())
    }

    /// Recovery on startup: move all inflight items back to pending, so they can be retried.
    /// This is needed in case the process was killed while processing some jobs, to avoid losing them.
    ///
    /// Note: this is a simple recovery mechanism that does not guarantee exactly-once processing,
    ///       but it is sufficient for many use cases. For more advanced scenarios, consider adding
    ///       timestamps and retry limits to the inflight items.
    ///
    /// Returns an error if the recovery process fails, or Ok(()) if it succeeds.
    ///
    /// Note: this method is called automatically when the queue is opened, so it should not be called
    ///       manually in normal operation.
    ///
    /// Example usage:
    /// ```
    /// let q = DiskPersistentQueue::open("/path/to/queue")?;
    /// q.recover()?;
    /// ```
    pub fn recover(&self) -> Result<(), SysinspectError> {
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

    /// Start a runner that continuously tries to dequeue and execute jobs using the provided async closure.
    /// The closure receives the job ID and the WorkItem, and should return a Future that completes when the job is done.
    /// The runner will automatically ack successful jobs. If the job fails, the caller is responsible for calling nack(id) inside the closure.
    ///
    /// This is a convenience method that wraps the start_ack() method, but does not handle nacking for you.
    ///
    /// Example usage:
    /// ```
    /// q.start(|job_id, item| async move {
    ///     // Process the item...
    ///     // Then ack or nack:
    ///     if success {
    ///         q.ack(job_id).unwrap();
    ///     } else {
    ///         q.nack(job_id).unwrap();
    ///     }
    /// });
    pub fn start<F, Fut>(&self, mut exec: F)
    where
        F: FnMut(u64, WorkItem) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.start_ack(move |id, item| {
            let fut = exec(id, item);
            async move {
                fut.await;
                Ok(())
            }
        });
    }

    /// Start a runner that continuously tries to dequeue and execute jobs using the provided async closure,
    /// and automatically acks successful jobs or nacks failed jobs. The closure receives the job ID and the WorkItem,
    /// and should return a Future that completes with a Result indicating whether the job was successful or not.
    ///
    /// Example usage:
    /// ```
    /// q.start_ack(|job_id, item| async move {
    ///     // Process the item...
    ///     // Return Ok(()) if successful, or Err(e) if failed:
    ///     if success {
    ///         Ok(())
    ///     } else {
    ///         Err(SysinspectError::new("Job failed"))
    ///     }
    /// });

    pub fn start_ack<F, Fut>(&self, mut exec: F) -> tokio::task::JoinHandle<()>
    where
        F: FnMut(u64, WorkItem) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), SysinspectError>> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let mut q = self.clone();
        q.tx = tx;

        tokio::spawn(async move {
            loop {
                loop {
                    match q.fetch() {
                        Ok(Some((id, item))) => match exec(id, item).await {
                            Ok(()) => {
                                if let Err(e) = q.ack(id) {
                                    log::error!("queue ack error for {id}: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("job {id} failed: {e}");
                                if let Err(e2) = q.nack(id) {
                                    log::error!("queue nack error for {id}: {e2}");
                                }
                            }
                        },
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
        })
    }
}
