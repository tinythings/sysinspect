use super::evthandler::EventHandler;
use crate::intp::{
    actproc::response::ActionResponse,
    conf::{EventConfig, EventConfigOption},
};
use colored::Colorize;
use libcommon::eidhub::get_eidhub;

#[derive(Default, Debug)]
pub struct ChainStopEventHandler {
    eid: String,
    config: EventConfig,
}

impl EventHandler for ChainStopEventHandler {
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        Self { eid, config: cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "chainstop".to_string()
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&Self::id())
    }

    fn handle(&self, evt: &ActionResponse) {
        if !evt.match_eid(&self.eid) {
            return;
        }

        let Some(cfg) = self.config() else {
            return;
        };

        // Preferred: eids: [ ... ]
        let mut targets: Vec<String> = vec![];
        if let Some(arr) = cfg.as_str_list("eids") {
            targets.extend(arr.into_iter().map(|s| s.to_string()));
        }
        let verbose = cfg.as_bool("verbose").unwrap_or(false);

        if targets.is_empty() {
            return;
        }

        tokio::spawn(async move {
            let hub = get_eidhub();
            for eid in targets {
                if verbose {
                    log::info!("[{}] Dropping EID: {}", Self::id().bright_blue(), eid);
                }
                hub.drop(&Self::id(), &eid).await;
            }
        });
    }
}
