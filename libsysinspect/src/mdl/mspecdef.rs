use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::SyspectError;

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

    // The rest of the system structure
    #[serde(flatten)]
    system: HashMap<String, Value>,
}

impl ModelSpec {
    fn find(&self, v: Value) -> Result<Value, SyspectError> {
        match v {
            /*
            Value::Sequence(v) => {
                for e in v.iter() {
                    self.find(e);
                }
            }
            */
            Value::Mapping(v) => {
                for (k, e) in v {
                    return Ok(self.find(e)?);
                }
            }
            Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                return Ok(v);
            }
            _ => {}
        }

        Err(SyspectError::ModelDSLError("Object not found".to_string()))
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
}
