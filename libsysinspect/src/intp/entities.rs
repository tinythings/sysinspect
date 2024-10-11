use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Claim {
    #[serde(flatten)]
    data: HashMap<String, Value>,
}

impl Claim {
    pub fn get(&self, name: &str) -> Option<&Value> {
        if let Some(v) = self.data.get(name) {
            return Some(v);
        }

        None
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Entity {
    descr: Option<String>,
    facts: Option<HashMap<String, Vec<Claim>>>,
    inherits: Option<Vec<String>>,
    depends: Option<Vec<String>>,

    // Value comes as key:data map with just one key.
    // This key is the ID, but cannot be parsed by serde at once.
    id: Option<String>,
}

impl Entity {
    pub fn new(id: &Value, data: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Entity::default();
        let eid: String;
        if let Some(id) = id.as_str() {
            eid = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("Entity has no ID".to_string()));
        }

        instance = serde_yaml::from_value::<Entity>(data.to_owned()).unwrap_or(instance);
        instance.id = Some(eid);

        Ok(instance)
    }

    /// Get the entity ID
    pub fn id(&self) -> String {
        self.id.clone().unwrap_or("".to_string())
    }

    /// Get entity dependencies
    pub fn depends(&self) -> Vec<String> {
        self.depends.to_owned().unwrap_or_default()
    }

    /// Get inherited entities that form this one
    pub fn inherits(&self) -> Vec<String> {
        self.inherits.to_owned().unwrap_or_default()
    }

    /// Return the description
    pub fn descr(&self) -> String {
        self.descr.to_owned().unwrap_or("".to_string())
    }

    /// Return facts
    pub fn facts(&self) -> Option<&HashMap<String, Vec<Claim>>> {
        self.facts.as_ref()
    }
}
