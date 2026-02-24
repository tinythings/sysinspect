use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct DataStorageConfig {
    expiration: Option<Duration>,
    max_item_size: Option<u64>,
    max_overall_size: Option<u64>,
}

impl DataStorageConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn expiration(mut self, d: Duration) -> Self {
        self.expiration = Some(d);
        self
    }

    pub fn max_item_size(mut self, bytes: u64) -> Self {
        self.max_item_size = Some(bytes);
        self
    }

    pub fn max_overall_size(mut self, bytes: u64) -> Self {
        self.max_overall_size = Some(bytes);
        self
    }

    pub fn get_max_overall_size(&self) -> Option<u64> {
        self.max_overall_size
    }

    pub fn get_max_item_size(&self) -> Option<u64> {
        self.max_item_size
    }

    pub fn get_expiration(&self) -> Option<Duration> {
        self.expiration
    }
}
