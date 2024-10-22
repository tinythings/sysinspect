pub mod cstr_stdhdl;
pub mod evthandler;
pub mod stdhdl;
use lazy_static::lazy_static;

#[allow(clippy::type_complexity)]
/// Handlers registry
///
/// To add a handler:
///
/// 1. Implement eventhandler::EventHandler trait
/// 2. Add into registry variable in init_handlers() function
pub mod registry {
    use crate::intp::conf::EventConfig;

    use super::*;
    use cstr_stdhdl::ConstraintHandler;
    use evthandler::EventHandler;
    use std::{
        collections::HashMap,
        sync::{Mutex, MutexGuard},
    };
    use stdhdl::StdoutEventHandler;

    lazy_static! {
        pub static ref REGISTRY_MAP: Mutex<HashMap<String, fn(String, EventConfig) -> Box<dyn EventHandler>>> =
            Mutex::new(HashMap::new());
    }

    pub fn init_handler(label: String, event_id: String, cfg: EventConfig) -> Option<Box<dyn EventHandler>> {
        let registry: MutexGuard<'_, HashMap<String, fn(String, EventConfig) -> Box<dyn EventHandler>>> =
            REGISTRY_MAP.lock().unwrap();
        if let Some(eh) = registry.get(&label) {
            return Some(eh(event_id, cfg));
        }

        None
    }

    pub fn init_handlers() {
        let mut registry = REGISTRY_MAP.lock().unwrap();
        if !registry.is_empty() {
            return;
        }

        // Handler registration
        log::debug!("Intialising handlers");
        registry.insert(StdoutEventHandler::id(), |eid, cfg| Box::new(StdoutEventHandler::new(eid, cfg)));
        registry.insert(ConstraintHandler::id(), |eid, cfg| Box::new(ConstraintHandler::new(eid, cfg)));
    }

    /// Get all registered handlers.
    /// NOTE: [`init_handlers`] must be called.
    ///
    pub fn get_handler_names() -> Vec<String> {
        REGISTRY_MAP.lock().unwrap().keys().map(|s| s.to_string()).collect::<Vec<String>>()
    }
}
