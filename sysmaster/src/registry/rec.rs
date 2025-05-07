use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

            if parts[1] == libsysinspect::util::dataconv::to_string(self.traits.get(parts[0]).cloned()).unwrap_or_default() {
                matched += 1;
            } else {
                log::error!("Selector {} does not match traits", selector);
                log::error!("{:#?}", self.traits);
                log::error!("------------");
            }
        }
        matched == set.len()
    }

    pub fn get_traits(&self) -> &HashMap<String, Value> {
        &self.traits
    }
}
