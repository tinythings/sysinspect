use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Relation {
    id: Option<String>,
    #[serde(flatten)]
    states: HashMap<String, HashMap<String, Vec<String>>>,
}

impl Relation {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Relation { id: None, states: HashMap::default() };

        if let Some(id) = id.as_str() {
            instance = serde_yaml::from_value::<Relation>(states.to_owned()).unwrap_or(instance);
            if instance.states.is_empty() {
                return Err(SysinspectError::ModelDSLError(
                    "No relations definitions were found or they are not in right syntax".to_string(),
                ));
            }
            instance.id = Some(id.to_string());
        } else {
            return Err(SysinspectError::ModelDSLError("Relation ID was not found".to_string()));
        }

        Ok(instance)
    }

    /// Get the relation ID
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }
}
