pub mod pulse;

use pulse::EventsScheduler;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SchedulerService {
    pulse: Arc<Mutex<EventsScheduler>>,
}

impl Default for SchedulerService {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerService {
    pub fn new() -> Self {
        Self { pulse: Arc::new(Mutex::new(EventsScheduler::new())) }
    }

    pub async fn add_event(&self, event: pulse::EventTask) -> Result<(), libsysinspect::SysinspectError> {
        self.pulse.lock().await.add(event).await
    }

    pub async fn remove_event(&self, id: &str) -> Result<(), libsysinspect::SysinspectError> {
        self.pulse.lock().await.remove(id).await
    }

    pub async fn start(&self) -> Result<(), libsysinspect::SysinspectError> {
        self.pulse.lock().await.start().await
    }
}
