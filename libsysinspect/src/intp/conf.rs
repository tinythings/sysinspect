use crate::{util, SysinspectError};
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
        util::dataconv::as_str_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as an integer
    pub fn as_int(&self, cfg: &str) -> Option<i64> {
        util::dataconv::as_int_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as an integer
    pub fn as_bool(&self, cfg: &str) -> Option<bool> {
        util::dataconv::as_bool_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as a vector of strings
    pub fn as_str_list(&self, cfg: &str) -> Option<Vec<String>> {
        util::dataconv::as_str_list_opt(self.data.get(cfg).cloned())
    }
}

/// A configuration of an event. It contains an array of
/// binding handlers and their configurations, respectfully.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EventConfig {
    handler: Vec<String>,
    #[serde(flatten)]
    cfg: Option<HashMap<String, EventConfigOption>>,
}

impl EventConfig {
    /// Get an event configuration for a handler, if any
    pub fn for_handler(&self, handler: &str) -> Option<EventConfigOption> {
        if let Some(cfg) = &self.cfg {
            if let Some(cfg) = cfg.get(handler) {
                return Some(cfg.to_owned());
            }
        }
        None
    }

    /// Get all handlers that are bound to
    pub(crate) fn get_bound_handlers(&self) -> &Vec<String> {
        &self.handler
    }

    /// Get specific event configuration
    pub fn cfg(&self) -> &Option<HashMap<String, EventConfigOption>> {
        &self.cfg
    }
}

/// The entire config
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    modules: PathBuf,

    // EventId to config, added later
    events: Option<HashMap<String, EventConfig>>,
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
        if let Ok(cfg) = serde_yaml::from_value::<HashMap<String, EventConfig>>(obj.to_owned()) {
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
    pub fn get_event(&self, event_id: &str) -> Option<EventConfig> {
        if let Some(e) = &self.events {
            return e.get(event_id).cloned();
        }

        None
    }

    /// Get all events Ids
    pub fn get_event_ids(&self) -> Vec<String> {
        if let Some(events) = &self.events {
            return events.keys().map(|s| s.to_owned()).collect::<Vec<String>>();
        }

        vec![]
    }
}
