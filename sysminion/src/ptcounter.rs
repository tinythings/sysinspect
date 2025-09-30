use procfs::Current;

/// Process and task counter.
/// This is used to keep track of the number of processes and tasks
/// running on the minion, to avoid overloading it.

#[derive(Debug, Clone)]
pub struct PTCounter {
    tasks: usize,
    loadaverage: f32,
}

impl PTCounter {
    /// Create new counter
    pub fn new() -> Self {
        Self { tasks: 0, loadaverage: 0.0 }
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
    pub fn inc(&mut self) {
        self.update_stats();
        self.tasks += 1;
    }

    /// Decrement task counter
    pub fn dec(&mut self) {
        if self.tasks > 0 {
            self.tasks -= 1;
        }
        self.update_stats();
    }

    /// Get current task count
    pub fn get_tasks(&self) -> usize {
        self.tasks
    }

    /// Get current load average (5 min)
    pub fn get_loadaverage(&self) -> f32 {
        self.loadaverage
    }
}
