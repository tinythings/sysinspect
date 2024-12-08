/*
Constraint result handler for STDOUT
 */

use super::evthandler::EventHandler;
use crate::intp::{
    actproc::response::ActionResponse,
    conf::{EventConfig, EventConfigOption},
};
use colored::Colorize;

#[derive(Default, Debug)]
pub struct ConstraintHandler {
    eid: String,
    config: EventConfig,
}

impl ConstraintHandler {
    /// Get prefix from the configuration
    fn get_prefix(&self) -> String {
        let mut prefix = "".to_string();
        if let Some(config) = self.config() {
            if let Some(p) = config.as_string("prefix") {
                prefix = format!("{} - ", p.cyan());
            }
        }

        prefix
    }
}

/// STDOUT event handler. It just outputs the action response to a log.
impl EventHandler for ConstraintHandler {
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        ConstraintHandler { eid, config: cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "outcome-logger".to_string()
    }

    fn handle(&self, evt: &ActionResponse) {
        if !&evt.match_eid(&self.eid) {
            return;
        }

        let prefix = self.get_prefix();

        if !evt.constraints.has_errors() {
            log::info!("{}{} {}", prefix, evt.aid(), "passed".bright_green().bold());
            return;
        }

        for f in evt.constraints.failures() {
            log::error!("{}{}: {}", prefix, f.title.yellow(), f.msg);
        }
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&ConstraintHandler::id())
    }
}
