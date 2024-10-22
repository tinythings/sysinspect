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

        if !evt.errors.has_errors() {
            log::info!("All constraints {}", "passed".bright_green().bold());
            return;
        }

        log::info!("{}", evt.errors.descr());
        for f in evt.errors.failures() {
            log::error!("{}: {}", f.title.yellow(), f.msg);
        }
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&ConstraintHandler::id())
    }
}
