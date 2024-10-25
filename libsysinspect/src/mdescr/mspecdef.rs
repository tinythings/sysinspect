use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, path::PathBuf};

/// Model Specification
/// ===================
///
/// This module's job is to read all the model files and put them together
/// into one tree, resolving all interpolative points to one single
/// configuration (spec)
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSpec {
    // These are fields of model.cfg init config
    //
    // Model name
    name: String,

    // Model version
    version: String,

    // A multi-line description of the model, used for reports
    // or other places.
    description: String,

    // Model maintainer
    maintainer: String,

    inherits: Option<String>,

    // The rest of the system structure
    #[serde(flatten)]
    system: HashMap<String, Value>,
}

impl ModelSpec {
    /// Get a top-level DSL section
    pub fn top(&self, id: &str) -> Option<&Value> {
        if self.system.contains_key(id) {
            if let Some(v) = self.system.get(id) {
                return Some(v);
            }
        }

        None
    }

    #[allow(clippy::only_used_in_recursion)]
    fn find(&self, v: Value) -> Result<Value, SysinspectError> {
        match v {
            /*
            Value::Sequence(v) => {
                for e in v.iter() {
                    self.find(e);
                }
            }
            */
            Value::Mapping(v) => {
                if let Some((k, e)) = v.into_iter().next() {
                    return self.find(e);
                }
            }
            Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                return Ok(v);
            }
            _ => {}
        }

        Err(SysinspectError::ModelDSLError("Object not found".to_string()))
    }

    /// Traverse the namespace by "foo:bar:person.name" syntax.
    /// "foo:bar" is a namespace, "person" is an ID of the object and "name" is a property.
    /// This yields to the following YAML:
    ///
    /// ```yaml
    /// foo:
    ///   bar:
    ///     id: person
    ///     name: Jeff
    /// ```
    pub fn traverse(&self, path: String) {
        if path.contains(".") && path.contains(":") {}
    }

    /// Returns Path to the inherited model
    pub fn inherits(&self) -> Option<PathBuf> {
        if let Some(p) = &self.inherits {
            return Some(PathBuf::from(p));
        }

        None
    }
}
