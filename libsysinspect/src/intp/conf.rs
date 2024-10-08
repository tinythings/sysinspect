use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    modules: PathBuf,
}

impl Config {
    pub fn new(obj: &Value) -> Result<Self, SysinspectError> {
        if let Ok(instance) = serde_yaml::from_value::<Config>(obj.to_owned()) {
            return Ok(instance);
        }

        Err(SysinspectError::ModelDSLError("Unable to parse configuration".to_string()))
    }

    /// Get module from the namespace
    pub fn get_module(&self, namespace: &str) -> Result<PathBuf, SysinspectError> {
        // Fool-proof cleanup, likely a bad idea
        let modpath = self.modules.join(
            namespace
                .trim_start_matches('.')
                .trim_end_matches('.')
                .trim()
                .split('.')
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join("/"),
        );

        if !modpath.exists() {
            return Err(SysinspectError::ModuleError(format!("Module \"{}\" was not found at {:?}", namespace, modpath)));
        }

        Ok(modpath)
    }
}
