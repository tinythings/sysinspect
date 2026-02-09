use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Relation {
    id: Option<String>,
    #[serde(flatten)]
    states: IndexMap<String, IndexMap<String, Vec<String>>>,
}

impl Relation {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Relation { id: None, states: IndexMap::default() };

        if let Some(id) = id.as_str() {
            instance = serde_yaml::from_value::<Relation>(states.to_owned()).unwrap_or(instance);
            if instance.states.is_empty() {
                return Err(SysinspectError::ModelDSLError("No relations definitions were found or they are not in right syntax".to_string()));
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

    /// Get states to relations
    pub fn states(&self) -> &IndexMap<String, IndexMap<String, Vec<String>>> {
        &self.states
    }

    /// Get required entities (consists of).
    /// There is no clear distinction between "consists of" and "required".
    pub fn required(&self, state: &str) -> Result<Vec<String>, SysinspectError> {
        let mut out = Vec::default();
        if let Some(set) = self.states().get(state) {
            if let Some(required) = set.get("requires") {
                out.extend(required.iter().map(|s| s.to_string()));
            }
        } else {
            return Err(SysinspectError::ModelDSLError(format!(
                "No required entities has been found in the \"{}\" relation as the \"{state}\" state",
                self.id()
            )));
        }

        Ok(out)
    }
}
