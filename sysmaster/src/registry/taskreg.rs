use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

/*
Task Registry Module.

This module defines the TaskRegistry struct, which is responsible for managing
the registration and tracking of tasks within the sysmaster system.

Task Registry keeps track of active tasks, their states, and other relevant metadata.
It works like a simple key-value store where task IDs (Cycle ID or CID) are keys to array
of targeted minion IDs. A task considered done if it has no more "dangling" minion IDs.

Task Registry has two modes:
1. In-memory database, tracking current online minions only.
2. Persistent database for resuming tasks for offline minions after sysmaster restarts.
 */
#[derive(Debug)]
pub struct TaskRegistry {
    ongoing: Mutex<HashMap<String, HashSet<String>>>, // Map of task IDs to list of targeted minion IDs
}
impl TaskRegistry {
    pub fn new() -> Self {
        TaskRegistry { ongoing: Mutex::new(HashMap::new()) }
    }

    /// Register a new task with its targeted minion IDs, incrmenting
    pub fn register(&mut self, cid: &str, mids: Vec<String>) {
        let mut ongoing = match self.ongoing.lock() {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire lock for task registry: {}", e);
                return;
            }
        };

        ongoing.insert(cid.to_string(), mids.into_iter().collect());
        log::info!(">>> Registered task {cid} with targeted minions: {:#?}", ongoing.get(cid));
    }

    /// Deregister a minion ID from a task
    pub fn deregister(&mut self, cid: &str, mid: &str) {
        let mut ongoing = match self.ongoing.lock() {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire lock for task registry: {}", e);
                return;
            }
        };
        if let Some(minions) = ongoing.get_mut(cid) {
            minions.remove(mid);
            log::info!(">>> Deregistered minion {mid} from task {cid}. Remaining minions: {:#?}", minions);
            if minions.is_empty() {
                ongoing.remove(cid);
                log::info!(">>> Task {cid} completed and removed from registry");
            }
        }
    }

    pub fn flush(&mut self, mid: &str, cids: &Vec<String>) {
        for cid in cids {
            log::info!(">>> Flushing minion {mid} from task {cid}");
        }

        let mut ongoing = match self.ongoing.lock() {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire lock for task registry: {}", e);
                return;
            }
        };
        ongoing.retain(|cid, mids| {
            if mids.contains(mid) {
                log::info!(">>> Flushing minion {mid} from task {cid}");
                mids.remove(mid);
            }
            if mids.is_empty() {
                log::info!(">>> Task {cid} completed and removed from registry");
            }
            !mids.is_empty()
        });
    }

    /// Get list of tasks a minion is involved in
    pub fn minion_tasks(&self, mid: &str) -> Vec<String> {
        let ongoing = match self.ongoing.lock() {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire lock for task registry: {}", e);
                return Vec::new();
            }
        };

        let mut tasks = Vec::new();
        for (cid, mids) in ongoing.iter() {
            if mids.contains(mid) {
                tasks.push(cid.clone());
            }
        }
        tasks
    }
}
