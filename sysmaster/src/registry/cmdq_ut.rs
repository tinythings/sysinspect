use super::{MasterCommandQueue, MasterCommandQueueStats, MasterCommandState};
use libsysproto::{MasterMessage, MinionTarget, rqtypes::RequestType};
use serde_json::json;
use std::time::Duration;

fn reopen_queue_with_retry(path: &std::path::Path) -> MasterCommandQueue {
    let mut last_err = None;
    for _ in 0..20 {
        match MasterCommandQueue::open(path) {
            Ok(queue) => return queue,
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    panic!("failed to reopen queue after retries: {}", last_err.unwrap());
}

fn queued_message(cycle_target: &str) -> MasterMessage {
    let mut msg = MasterMessage::new(RequestType::Command, json!({"uri":"model://demo","files":{},"models_root":"models"}));
    msg.set_target(MinionTarget::new(cycle_target, ""));
    msg
}

#[test]
fn enqueue_persists_across_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = MasterCommandQueue::open(tmp.path()).unwrap();
    let msg = queued_message("minion-1");
    let cycle_id = msg.cycle().clone();

    let id = queue.enqueue("minion-1", &msg).unwrap();
    drop(queue);

    let reopened = reopen_queue_with_retry(tmp.path());
    let pending = reopened.pending_for_minion("minion-1").unwrap();

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id(), id);
    assert_eq!(pending[0].replay_key(), format!("mcmd|minion-1|{cycle_id}"));
    assert_eq!(pending[0].state(), MasterCommandState::Pending);
    assert_eq!(pending[0].message().cycle(), &cycle_id);
}

#[test]
fn pending_for_minion_returns_enqueue_order() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = MasterCommandQueue::open(tmp.path()).unwrap();
    let first = queued_message("minion-1");
    let second = queued_message("minion-1");
    let third = queued_message("minion-2");

    let id1 = queue.enqueue("minion-1", &first).unwrap();
    let id2 = queue.enqueue("minion-1", &second).unwrap();
    let _ = queue.enqueue("minion-2", &third).unwrap();

    let pending = queue.pending_for_minion("minion-1").unwrap();
    assert_eq!(pending.iter().map(|entry| entry.id()).collect::<Vec<u64>>(), vec![id1, id2]);
}

#[test]
fn stats_report_pending_replayed_and_distinct_minions() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = MasterCommandQueue::open(tmp.path()).unwrap();
    let first = queued_message("minion-1");
    let second = queued_message("minion-1");
    let third = queued_message("minion-2");

    let id1 = queue.enqueue("minion-1", &first).unwrap();
    let id2 = queue.enqueue("minion-1", &second).unwrap();
    let _ = queue.enqueue("minion-2", &third).unwrap();
    assert!(queue.set_state(id2, MasterCommandState::Replayed).unwrap());
    assert!(queue.set_state(id1, MasterCommandState::Cleared).unwrap());

    assert_eq!(queue.stats().unwrap(), MasterCommandQueueStats { pending_commands: 1, replayed_commands: 1, queued_minions: 2 });
}

#[test]
fn remove_drops_command_from_backlog() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = MasterCommandQueue::open(tmp.path()).unwrap();
    let msg = queued_message("minion-1");
    let id = queue.enqueue("minion-1", &msg).unwrap();

    assert!(queue.remove(id).unwrap());
    assert!(!queue.remove(id).unwrap());
    assert!(queue.pending_for_minion("minion-1").unwrap().is_empty());
}

#[test]
fn remove_by_replay_key_clears_matching_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let queue = MasterCommandQueue::open(tmp.path()).unwrap();
    let first = queued_message("minion-1");
    let second = queued_message("minion-1");
    let key = format!("mcmd|minion-1|{}", first.cycle());

    let _ = queue.enqueue("minion-1", &first).unwrap();
    let _ = queue.enqueue("minion-1", &second).unwrap();

    assert_eq!(queue.remove_by_replay_key(&key).unwrap(), 1);
    let pending = queue.pending_for_minion("minion-1").unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message().cycle(), second.cycle());
}
