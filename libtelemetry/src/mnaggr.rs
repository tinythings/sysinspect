use serde_json::Value as JsonValue;

pub struct MinionAggregator {
    label: String,
    data: JsonValue,
}

impl MinionAggregator {
    pub fn new(label: String) -> Self {
        MinionAggregator { label, data: JsonValue::Null }
    }

    /// Add minion's data to the aggregator
    pub fn add_data(&mut self, data: JsonValue) {
        self.data = data;
    }

    /// Aggregate all the data from the minion for the current cycle.
    /// Returns an array of JSON objects, each containing a data for the OTLP log entry.
    pub fn aggregate(&self) -> Vec<JsonValue> {
        vec![self.data.clone()]
    }
}
