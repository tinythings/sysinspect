use super::evthandler::EventHandler;
use crate::{
    intp::{actproc::response::ActionResponse, conf::EventConfig},
    reactor::fmt::{formatter::StringFormatter, kvfmt::KeyValueFormatter},
};

#[derive(Default, Debug)]
pub struct StdoutEventHandler {
    event_ids: Vec<String>,
    config: EventConfig,
}

/// STDOUT event handler. It just outputs the action response to a log.
impl StdoutEventHandler {
    /// Bind an event Id to this handler
    pub fn bind_event_id(&mut self, evt_id: &str) {
        self.event_ids.push(evt_id.to_string());
    }
}

impl EventHandler for StdoutEventHandler {
    /// Create an event handler
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        StdoutEventHandler { event_ids: vec![], config: cfg }
    }

    fn handle(&self, evt: &ActionResponse) {
        if self.event_ids.contains(&evt.event_name()) {
            log::debug!("No event for \"{}\" registered", evt.event_name());
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
}
