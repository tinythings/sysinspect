pub mod cstr_stdhdl;
pub mod evthandler;
pub mod pipeline;
pub mod pipescript;
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
    use super::*;
    use crate::intp::conf::EventConfig;
    use cstr_stdhdl::ConstraintHandler;
    use dashmap::DashMap;
    use evthandler::EventHandler;
    use pipescript::PipeScriptHandler;
    use stdhdl::StdoutEventHandler;

    lazy_static! {
        pub static ref REGISTRY_MAP: DashMap<String, fn(String, EventConfig) -> Box<dyn EventHandler>> = DashMap::new();
    }

    pub fn init_handler(label: String, event_id: String, cfg: EventConfig) -> Option<Box<dyn EventHandler>> {
        if let Some(eh) = REGISTRY_MAP.get(&label) {
            return Some(eh(event_id, cfg));
        }

        None
    }

    pub fn init_handlers() {
        if !REGISTRY_MAP.is_empty() {
            return;
        }

        // Handler registration
        log::debug!("Intialising handlers");
        REGISTRY_MAP.insert(StdoutEventHandler::id(), |eid, cfg| Box::new(StdoutEventHandler::new(eid, cfg)));
        REGISTRY_MAP.insert(ConstraintHandler::id(), |eid, cfg| Box::new(ConstraintHandler::new(eid, cfg)));
        REGISTRY_MAP.insert(PipeScriptHandler::id(), |eid, cfg| Box::new(PipeScriptHandler::new(eid, cfg)));
    }

    /// Get all registered handlers.
    /// NOTE: [`init_handlers`] must be called.
    ///
    pub fn get_handler_names() -> Vec<String> {
        let mut out = REGISTRY_MAP.iter().map(|entry| entry.key().clone()).collect::<Vec<String>>();
        out.sort();

        out
    }
}
