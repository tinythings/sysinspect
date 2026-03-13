use crate::{
    cfg::mmconf::MinionConfig,
    traits::{get_minion_traits_nolog, systraits::SystemTraits},
};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Value, json};

/// Stable host context shared with all runtime backends.
///
/// `traits` is the primary host-facts surface. It is intentionally left dynamic,
/// because traits can come from multiple sources, including user-controlled ones.
/// `paths` and `capabilities` are small convenience sections, but they are not a
/// replacement for reading host facts from `traits`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuntimeHostContext {
    traits: IndexMap<String, Value>,
    paths: RuntimeHostPaths,
    capabilities: IndexMap<String, Value>,
}

/// Runtime-relevant host paths.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct RuntimeHostPaths {
    sharelib: String,
    root: String,
    models: String,
    traits: String,
    functions: String,
    sensors: String,
}

/// Build the shared runtime host context from minion config and traits.
pub fn build_runtime_host_context(cfg: &MinionConfig, traits: &SystemTraits) -> RuntimeHostContext {
    RuntimeHostContext {
        traits: traits.to_map(),
        paths: RuntimeHostPaths {
            sharelib: cfg.sharelib_dir().display().to_string(),
            root: cfg.root_dir().display().to_string(),
            models: cfg.models_dir().display().to_string(),
            traits: cfg.traits_dir().display().to_string(),
            functions: cfg.functions_dir().display().to_string(),
            sensors: cfg.sensors_dir().display().to_string(),
        },
        capabilities: IndexMap::from([("packagekit".to_string(), json!(false))]),
    }
}

/// Build the shared runtime host context as JSON.
pub fn get_runtime_host_context_json(cfg: &MinionConfig) -> Value {
    serde_json::to_value(build_runtime_host_context(cfg, &get_minion_traits_nolog(Some(cfg)))).unwrap_or_default()
}
