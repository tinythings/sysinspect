use super::evthandler::EventHandler;
use crate::intp::actproc::response::ActionResponse;

#[derive(Default)]
pub struct StdoutEventHandler {}

/// STDOUT event handler. It just outputs the action response to a log.
impl StdoutEventHandler {
    pub fn new() -> Self {
        StdoutEventHandler {}
    }
}

impl EventHandler for StdoutEventHandler {
    fn handle(&self, evt: &ActionResponse) {
        if evt.event_name() != "" {
            log::debug!("No event for \"{}\" registered", evt.event_name());
            return;
        }

        if evt.response.retcode() == 0 {
            log::info!("{}/{} - {}", evt.eid(), evt.aid(), evt.response.message());
        } else {
            log::error!("{}/{} ({}) - {}", evt.eid(), evt.aid(), evt.response.retcode(), evt.response.message());
        }

        // Dump also warning messages
        for wmsg in evt.response.warnings() {
            log::warn!("{}/{} - {}", evt.eid(), evt.aid(), wmsg);
        }
    }

    /// Handler Id
    fn id(&self) -> String {
        "console-logger".to_string()
    }
}
