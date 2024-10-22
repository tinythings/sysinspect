use super::evthandler::EventHandler;
use crate::{
    intp::{
        actproc::response::ActionResponse,
        conf::{EventConfig, EventConfigOption},
    },
    reactor::fmt::{formatter::StringFormatter, kvfmt::KeyValueFormatter},
};
use colored::Colorize;

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
        if !&evt.match_eid(&self.eid) {
            return;
        }

        let mut prefix = "".to_string();
        let mut verbose = true;
        if let Some(config) = self.config() {
            if let Some(p) = config.as_string("prefix") {
                prefix = format!("{} - ", p.cyan());
            }

            if let Some(p) = config.as_bool("concise") {
                verbose = !p;
            }
        }

        if evt.response.retcode() == 0 {
            log::info!("{}{}/{} - {}", prefix, evt.eid().bright_cyan(), evt.aid().bright_cyan(), evt.response.message());
            if verbose {
                if let Some(data) = evt.response.data() {
                    log::info!(
                        "{}{}/{} - Other data:\n{}",
                        prefix,
                        evt.eid().bright_cyan(),
                        evt.aid().bright_cyan(),
                        KeyValueFormatter::new(data).format()
                    );
                }
            }
        } else {
            log::error!(
                "{}{}/{} (Error: {}) - {}",
                prefix,
                evt.eid().bright_cyan(),
                evt.aid().bright_cyan(),
                evt.response.retcode(),
                evt.response.message()
            );
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

    /// Get confniguration of an event handler
    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&StdoutEventHandler::id())
    }
}
