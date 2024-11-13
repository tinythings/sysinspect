use std::{collections::HashMap, default};

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
        self.traits.get(attr).and_then(|f| Some(f.eq(&v))).unwrap_or(false)
    }

    pub fn get_traits(&self) -> &HashMap<String, Value> {
        &self.traits
    }
}
