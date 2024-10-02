use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, vec};

use crate::SysinspectError;

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    #[serde(flatten)]
    states: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Entity {
    descr: String,
    facts: Option<Claims>,
    inherits: Option<Vec<String>>,
    depends: Option<Vec<String>>,

    // Value comes as key:data map with just one key.
    // This key is the ID, but cannot be parsed by serde at once.
    id: Option<String>,
}

impl Entity {
    pub fn new(data: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Entity { facts: None, inherits: None, depends: None, id: None, descr: String::from("") };

        if let Some((id, data)) = data.as_mapping().unwrap().into_iter().next() {
            instance.id = Some(id.as_str().to_owned().unwrap().to_string());

            if let Some(datamap) = data.clone().as_mapping() {
                for (k, v) in datamap {
                    if let Some(dtv) = k.as_str() {
                        let v = v.clone();
                        if dtv == "facts" {
                            instance.facts = Some(serde_yaml::from_value(v).unwrap());
                        } else if dtv == "descr" || dtv == "description" {
                            instance.descr = serde_yaml::from_value(v).unwrap();
                        } else if dtv == "inherits" {
                            instance.inherits = serde_yaml::from_value(v).unwrap();
                        } else if dtv == "depends" {
                            instance.depends = serde_yaml::from_value(v).unwrap();
                        } else {
                            return Err(SysinspectError::ModelDSLError(format!("Unsupported entity directive: '{}'", dtv)));
                        }
                    }
                }
            }
        }

        Ok(instance)
    }

    /// Get the entity ID
    pub fn id(&self) -> String {
        self.id.clone().unwrap()
    }

    /// Get entity dependencies
    pub fn depends(&self) -> Vec<String> {
        if let Some(deps) = self.depends.clone() {
            return deps;
        }
        vec![]
    }

    /// Get inherited entities that form this one
    pub fn inherits(&self) -> Vec<String> {
        if let Some(inh) = self.inherits.clone() {
            return inh;
        }
        vec![]
    }

    // Return the description
    pub fn descr(&self) -> String {
        self.descr.to_owned()
    }
}
