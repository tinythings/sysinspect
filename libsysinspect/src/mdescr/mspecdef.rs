use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::PathBuf;

use super::telemetry::TelemetrySpec;

/// Model Specification
/// ===================
///
/// This module's job is to read all the model files and put them together
/// into one tree, resolving all interpolative points to one single
/// configuration (spec)
#[derive(Debug, Serialize, Deserialize, Clone)]
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

    telemetry: Option<TelemetrySpec>,

    // The rest of the system structure
    #[serde(flatten)]
    system: IndexMap<String, Value>,
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

    /// Returns Path to the inherited model
    pub fn inherits(&self) -> Option<PathBuf> {
        if let Some(p) = &self.inherits {
            return Some(PathBuf::from(p));
        }

        None
    }

    /// Get telemetry spec
    pub fn telemetry(&self) -> Option<TelemetrySpec> {
        if let Some(t) = &self.telemetry {
            return Some(t.clone());
        }

        None
    }
}
