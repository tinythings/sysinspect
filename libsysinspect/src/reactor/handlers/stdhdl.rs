use super::evthandler::EventHandler;
use crate::{
    intp::{
        actproc::response::ActionResponse,
        conf::{EventConfig, EventConfigOption},
    },
    reactor::fmt::{formatter::StringFormatter, kvfmt::KeyValueFormatter},
};
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct StdoutEventHandler {
    eid: String,
    config: EventConfig,
}

/// STDOUT event handler. It just outputs the action response to a log.
impl EventHandler for StdoutEventHandler {
    /// Create an event handler
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        StdoutEventHandler { eid, config: cfg }
    }

    fn handle(&self, evt: &ActionResponse) {
        if !self.eid.eq(&evt.event_name()) {
            return;
        }

        if evt.response.retcode() == 0 {
            log::info!("{}/{} - {}", evt.eid(), evt.aid(), evt.response.message());
            if let Some(data) = evt.response.data() {
                log::info!("{}/{} - Other data:\n{}", evt.eid(), evt.aid(), KeyValueFormatter::new(data).format());
            }
        } else {
            log::error!("{}/{} (Error: {}) - {}", evt.eid(), evt.aid(), evt.response.retcode(), evt.response.message());
        }

        // Dump also warning messages
        for wmsg in evt.response.warnings() {
            log::warn!("{}/{} - {}", evt.eid(), evt.aid(), wmsg);
        }
    }

    /// Return Id of the handler
    fn id() -> String
    where
        Self: Sized,
    {
        "console-logger".to_string()
    }

    fn config(&self) -> &Option<HashMap<String, EventConfigOption>> {
        self.config.cfg()
    }
}
