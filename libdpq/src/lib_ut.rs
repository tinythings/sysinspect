use crate::{DiskPersistentQueue, WorkItem};
use libsysproto::{MasterMessage, rqtypes::RequestType};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "libdpq-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        label
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn work_item() -> WorkItem {
    WorkItem::MasterCommand(MasterMessage::new(RequestType::Command, json!({"uri": "model://test"})))
}

#[test]
fn fetch_moves_job_to_inflight_and_ack_clears_it() {
    let dir = temp_dir("fetch-ack");
    let q = DiskPersistentQueue::open(&dir).unwrap();
    let id = q.add(work_item()).unwrap();

    let fetched = q.fetch().unwrap();
    assert!(matches!(fetched, Some((job_id, _)) if job_id == id));
    assert!(q.pending.is_empty());
    assert_eq!(q.inflight.len(), 1);
    assert_eq!(q.jobs.len(), 1);

    q.ack(id).unwrap();
    assert!(q.pending.is_empty());
    assert!(q.inflight.is_empty());
    assert!(q.jobs.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_recovers_inflight_job_back_to_pending() {
    let dir = temp_dir("recover-inflight");
    let id = {
        let q = DiskPersistentQueue::open(&dir).unwrap();
        let id = q.add(work_item()).unwrap();
        let fetched = q.fetch().unwrap();
        assert!(matches!(fetched, Some((job_id, _)) if job_id == id));
        assert!(q.pending.is_empty());
        assert_eq!(q.inflight.len(), 1);
        id
    };

    let q = DiskPersistentQueue::open(&dir).unwrap();
    assert_eq!(q.pending.len(), 1);
    assert!(q.inflight.is_empty());
    assert_eq!(q.jobs.len(), 1);
    let fetched = q.fetch().unwrap();
    assert!(matches!(fetched, Some((job_id, _)) if job_id == id));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_after_ack_does_not_requeue_completed_job() {
    let dir = temp_dir("reopen-acked");
    {
        let q = DiskPersistentQueue::open(&dir).unwrap();
        let id = q.add(work_item()).unwrap();
        let fetched = q.fetch().unwrap();
        assert!(matches!(fetched, Some((job_id, _)) if job_id == id));
        q.ack(id).unwrap();
    }

    let q = DiskPersistentQueue::open(&dir).unwrap();
    assert!(q.pending.is_empty());
    assert!(q.inflight.is_empty());
    assert!(q.jobs.is_empty());
    assert!(q.fetch().unwrap().is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn aborting_runner_leaves_job_recoverable_on_reopen() {
    let dir = temp_dir("abort-recover");
    let q = DiskPersistentQueue::open(&dir).unwrap();
    let id = q.add(work_item()).unwrap();

    let started = Arc::new(AtomicBool::new(false));
    let (hold_tx, hold_rx) = oneshot::channel::<()>();
    let hold_rx = Arc::new(tokio::sync::Mutex::new(Some(hold_rx)));

    let runner = q.start_ack({
        let started = Arc::clone(&started);
        let hold_rx = Arc::clone(&hold_rx);
        move |_job_id, _item| {
            let started = Arc::clone(&started);
            let hold_rx = Arc::clone(&hold_rx);
            async move {
                started.store(true, Ordering::Relaxed);
                if let Some(rx) = hold_rx.lock().await.take() {
                    let _ = rx.await;
                }
                Ok(())
            }
        }
    });

    timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("runner never started processing job");

    runner.abort();
    let _ = runner.await;
    drop(hold_tx);
    drop(q);

    let q = DiskPersistentQueue::open(&dir).unwrap();
    assert_eq!(q.pending.len(), 1);
    assert!(q.inflight.is_empty());
    let fetched = q.fetch().unwrap();
    assert!(matches!(fetched, Some((job_id, _)) if job_id == id));

    let _ = std::fs::remove_dir_all(&dir);
}
