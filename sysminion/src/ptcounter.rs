use std::collections::HashSet;

use procfs::Current;

/// Process and task counter.
/// This is used to keep track of the number of processes and tasks
/// running on the minion, to avoid overloading it.
///
/// Counter is also keeping track of done tasks and sends their cycle IDs
/// back to the master for bookkeeping when it reaches zero. Master can then
/// figure out what tasks were dropped/missed, if any.
#[derive(Debug, Clone)]
pub struct PTCounter {
    loadaverage: f32,
    tasks: HashSet<String>, // Cycle IDs of added tasks
    done: HashSet<String>,  // Cycle IDs of done tasks
}

impl PTCounter {
    /// Create new counter
    pub fn new() -> Self {
        Self { tasks: HashSet::new(), done: HashSet::new(), loadaverage: 0.0 }
    }

    /// Update load average from /proc/loadavg
    /// This is called internally on each inc/dec operation.
    /// More stats will/should be added here.
    fn update_stats(&mut self) {
        if let Ok(la) = procfs::LoadAverage::current() {
            self.loadaverage = la.five;
        }
    }

    /// Increment task counter
    pub fn inc(&mut self, cid: &str) {
        self.update_stats();
        self.tasks.insert(cid.to_string());
        log::info!("Added task {}, count increased to {}, load average: {}", cid, self.tasks.len(), self.loadaverage);
    }

    /// Decrement task counter
    pub fn dec(&mut self, cid: &str) {
        self.tasks.remove(cid);
        self.done.insert(cid.to_string());
        self.update_stats();
        log::info!("Removed task {}, count decreased to {}, load average: {}", cid, self.tasks.len(), self.loadaverage);
    }

    /// Get current task count
    pub fn is_done(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get done task ids
    /// This also clears the done set.
    pub fn get_done(&mut self) -> Vec<String> {
        self.done.iter().cloned().collect::<Vec<String>>()
    }

    pub fn flush_done(&mut self) {
        self.done.clear();
    }

    /// Get current load average (5 min)
    pub fn get_loadaverage(&self) -> f32 {
        self.loadaverage
    }
}
