pub struct LogEvent {}

impl Default for LogEvent {
    fn default() -> Self {
        Self::new()
    }
}

impl LogEvent {
    pub fn new() -> Self {
        LogEvent {}
    }
}
