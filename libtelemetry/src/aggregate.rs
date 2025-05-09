use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Aggregate telemetry data from all minions
pub struct CycleAggregator {
    // Minion label to minion data in JSON
    // Label is constructed from traits.
    minions: HashMap<String, JsonValue>,
}

impl Default for CycleAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl CycleAggregator {
    pub fn new() -> Self {
        CycleAggregator { minions: HashMap::new() }
    }

    /// Add minion's data to the aggregator
    pub fn add_data(&mut self, label: String, data: JsonValue) {
        self.minions.insert(label, data);
    }

    /// Aggregate all the data from minions for the current cycle.
    /// Returns an array of JSON objects, each containg a data for the OTLP log entry.
    pub fn aggregate(&self) -> Vec<JsonValue> {
        let mut aggregated_data = JsonValue::Object(serde_json::Map::new());

        for (label, data) in &self.minions {
            aggregated_data[label] = data.clone();
        }

        vec![aggregated_data]
    }
}

pub struct MinionAggregator {
    label: String,
    traits: HashMap<String, JsonValue>,
    data: JsonValue,
}

impl MinionAggregator {
    pub fn new(label: String, traits: HashMap<String, JsonValue>) -> Self {
        MinionAggregator { label, data: JsonValue::Null, traits }
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
