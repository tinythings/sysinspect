use std::collections::{HashMap, HashSet};

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
    ongoing: HashMap<String, HashSet<String>>, // Map of task IDs to list of targeted minion IDs
}
impl TaskRegistry {
    pub fn new() -> Self {
        TaskRegistry { ongoing: HashMap::new() }
    }

    /// Register a new task with its targeted minion IDs, incrmenting
    pub fn register(&mut self, cid: &str, mids: Vec<String>) {
        self.ongoing.insert(cid.to_string(), mids.into_iter().collect());
        log::info!(">>> Registered task {cid} with targeted minions: {:#?}", self.ongoing.get(cid));
    }

    /// Deregister a minion ID from a task
    pub fn deregister(&mut self, cid: &str, mid: &str) {
        if let Some(minions) = self.ongoing.get_mut(cid) {
            minions.remove(mid);
            log::info!(">>> Deregistered minion {mid} from task {cid}. Remaining minions: {:#?}", minions);
            if minions.is_empty() {
                self.ongoing.remove(cid);
                log::info!(">>> Task {cid} completed and removed from registry");
            }
        }
    }

    /// Get list of tasks a minion is involved in
    pub fn minion_tasks(&self, mid: &str) -> Vec<String> {
        let mut tasks = Vec::new();
        for (cid, mids) in self.ongoing.iter() {
            if mids.contains(mid) {
                tasks.push(cid.clone());
            }
        }
        tasks
    }
}
