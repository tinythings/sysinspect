use super::inspector::get_cfg_sharelib;
use crate::{
    SysinspectError,
    cfg::mmconf::DEFAULT_MODULES_DIR,
    util::dataconv::{as_bool_opt, as_int_opt, as_str_list_opt, as_str_opt},
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EventConfigOption {
    #[serde(flatten)]
    data: IndexMap<String, Value>,
}

impl EventConfigOption {
    /// Get an option as a string type
    pub fn as_string(&self, cfg: &str) -> Option<String> {
        as_str_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as an integer
    pub fn as_int(&self, cfg: &str) -> Option<i64> {
        as_int_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as an integer
    pub fn as_bool(&self, cfg: &str) -> Option<bool> {
        as_bool_opt(self.data.get(cfg).cloned())
    }

    /// Get an option as a vector of strings
    pub fn as_str_list(&self, cfg: &str) -> Option<Vec<String>> {
        as_str_list_opt(self.data.get(cfg).cloned())
    }
}

/// A configuration of an event. It contains an array of
/// binding handlers and their configurations, respectfully.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EventConfig {
    handlers: Vec<String>,
    #[serde(flatten)]
    cfg: Option<IndexMap<String, EventConfigOption>>,
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
        &self.handlers
    }

    /// Get specific event configuration by a handler Id (`hid`).
    pub fn cfg(&self, hid: &str) -> Option<EventConfigOption> {
        if let Some(cfg) = &self.cfg {
            return cfg.get(hid).cloned();
        }

        None
    }
}

/// The entire config
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    modules: Option<PathBuf>,

    // EventId to config, added later
    events: Option<IndexMap<String, EventConfig>>,
}

impl Config {
    pub fn new(obj: &Value) -> Result<Self, SysinspectError> {
        if let Ok(instance) = serde_yaml::from_value::<Config>(obj.to_owned()) {
            return Ok(instance);
        }

        Err(SysinspectError::ModelDSLError("Unable to parse configuration".to_string()))
    }

    /// Get module (or Python module) from the namespace
    pub fn get_module(&self, namespace: &str) -> Result<PathBuf, SysinspectError> {
        // Fool-proof cleanup, likely a bad idea
        // XXX: This is reimplemented in modfinder::ModCall::set_module_ns
        let mut modpath = self.modules.to_owned().unwrap_or(get_cfg_sharelib().join(DEFAULT_MODULES_DIR)).join(
            namespace.trim_start_matches('.').trim_end_matches('.').trim().split('.').map(|s| s.to_string()).collect::<Vec<String>>().join("/"),
        );

        let pymodpath = modpath.parent().unwrap().join(format!("{}.py", modpath.file_name().unwrap().to_os_string().to_str().unwrap_or_default()));

        // Collision
        if pymodpath.exists() && modpath.exists() {
            return Err(SysinspectError::ModuleError(format!(
                "Module names must be unique, however both \"{}\" and \"{}\" do exist. Please rename one of these, update your model and continue.",
                pymodpath.file_name().unwrap_or_default().to_str().unwrap_or_default(),
                modpath.file_name().unwrap_or_default().to_str().unwrap_or_default()
            )));
        }

        if !modpath.exists() {
            if !pymodpath.exists() {
                return Err(SysinspectError::ModuleError(format!(
                    "Missing module \"{}\" in \"{}\"",
                    namespace,
                    modpath.to_str().unwrap_or_default()
                )));
            } else {
                modpath = pymodpath;
            }
        }

        Ok(modpath.to_owned())
    }

    /// Set events config
    pub(crate) fn set_events(&mut self, obj: &Value) -> Result<(), SysinspectError> {
        if let Ok(cfg) = serde_yaml::from_value::<IndexMap<String, EventConfig>>(obj.to_owned()) {
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
