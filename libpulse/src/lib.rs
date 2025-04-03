mod pulse;

use pulse::EventsPulse;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PulseService {
    pulse: Arc<Mutex<EventsPulse>>,
}
