use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModArgs {
    opts: Option<Vec<String>>,
    args: Option<Vec<HashMap<String, String>>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Action {
    id: Option<String>,
    description: Option<String>,
    module: String,
    bind: Vec<String>,
    state: HashMap<String, ModArgs>,
}

impl Action {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Action::default();
        let i_id: String;

        if let Some(id) = id.as_str() {
            i_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("No id found for an action".to_string()));
        }

        if let Ok(mut i) = serde_yaml::from_value::<Action>(states.to_owned()) {
            i.id = Some(i_id);
            instance = i;
        }

        Ok(instance)
    }

    /// Get action's `id`
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Returns true if an action has a bind to an entity via its `eid` _(entity Id)_.
    pub fn binds_to(&self, eid: &str) -> bool {
        self.bind.contains(&eid.to_string())
    }
}
