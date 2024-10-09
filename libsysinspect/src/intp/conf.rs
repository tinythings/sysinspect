use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EventConfigOption {
    #[serde(flatten)]
    data: HashMap<String, Value>,
}

impl EventConfigOption {
    /// Get an option as a string type
    pub fn as_string(&self, cfg: &str) -> Option<String> {
        if let Some(v) = self.data.get(cfg) {
            match v {
                Value::String(v) => return Some(v.to_owned()),
                _ => {}
            }
        }

        None
    }

    /// Get an option as an integer
    pub fn as_int(&self, cfg: &str) -> Option<i64> {
        if let Some(v) = self.data.get(cfg) {
            match v {
                Value::Number(v) => {
                    if let Some(v) = v.as_i64() {
                        return Some(v);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Get an option as an integer
    pub fn as_bool(&self, cfg: &str) -> Option<bool> {
        if let Some(v) = self.data.get(cfg) {
            match v {
                Value::Bool(v) => return Some(v.to_owned()),
                _ => {}
            }
        }
        None
    }

    /// Get an option as a vector of strings
    pub fn as_str_list(&self, cfg: &str) -> Option<Vec<String>> {
        if let Some(v) = self.data.get(cfg) {
            match v {
                Value::Sequence(v) => {
                    let mut out: Vec<String> = Vec::default();
                    for i in v {
                        if let Some(i) = i.as_str() {
                            out.push(i.to_string());
                        }
                    }
                    return if v.len() == out.len() { Some(out) } else { None };
                }
                _ => {}
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EventsConfig {
    handler: Vec<String>,
    #[serde(flatten)]
    cfg: Option<HashMap<String, EventConfigOption>>,
}

impl EventsConfig {
    pub fn for_handler(&self, handler: &str) -> Option<EventConfigOption> {
        if let Some(cfg) = &self.cfg {
            if let Some(cfg) = cfg.get(handler) {
                return Some(cfg.to_owned());
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    modules: PathBuf,

    // EventId to config, added later
    events: Option<HashMap<String, EventsConfig>>,
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

    /// Set events config
    pub(crate) fn set_events(&mut self, obj: &Value) -> Result<(), SysinspectError> {
        if let Ok(cfg) = serde_yaml::from_value::<HashMap<String, EventsConfig>>(obj.to_owned()) {
            self.events = Some(cfg);
        } else {
            return Err(SysinspectError::ModelDSLError("Events configuration error".to_string()));
        }

        Ok(())
    }

    /// Get an event config by event id.
    /// An event Id is constructed from three parts as a path:
    ///
    /// `<action-id>/<bound entity/<state>`
    ///
    /// State can be default, i.e. `$`.
    pub fn get_event(&self, event_id: &str) -> Option<EventsConfig> {
        if let Some(e) = &self.events {
            return e.get(event_id).cloned();
        }

        None
    }
}
