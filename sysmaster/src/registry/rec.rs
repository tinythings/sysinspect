use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MinionRecord {
    id: String,
    traits: HashMap<String, Value>,
}

impl MinionRecord {
    pub fn new(id: String, traits: HashMap<String, Value>) -> Self {
        MinionRecord { id, traits }
    }

    /// Check if the record matches the value
    pub fn matches(&self, attr: &str, v: Value) -> bool {
        self.traits.get(attr).map(|f| f.eq(&v)).unwrap_or(false)
    }

    // Get minion id
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn matches_selectors(&self, set: Vec<String>) -> bool {
        if set.is_empty() || set.contains(&"*".to_string()) {
            return true;
        }

        let mut matched = 0;
        for selector in &set {
            if !selector.contains(":") {
                log::warn!("Invalid selector format: {}", selector);
                continue;
            }

            let parts: Vec<&str> = selector.split(':').collect(); // attr:value
            if parts.len() != 2 {
                log::warn!("Invalid selector format: {}", selector);
                continue;
            }

            if libtelemetry::expr::expr(parts[1], self.traits.get(parts[0]).cloned().unwrap_or_default()) {
                matched += 1;
            }
        }
        matched == set.len()
    }

    pub fn get_traits(&self) -> &HashMap<String, Value> {
        &self.traits
    }
}
