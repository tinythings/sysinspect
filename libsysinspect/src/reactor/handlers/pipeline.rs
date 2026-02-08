use crate::{
    intp::{
        actproc::response::ActionResponse,
        conf::{EventConfig, EventConfigOption},
    },
    reactor::handlers::evthandler::EventHandler,
};

#[derive(Default, Debug)]
pub struct PipelineHandler {
    eid: String,
    config: EventConfig,
}

impl EventHandler for PipelineHandler {
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        PipelineHandler { eid, config: cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "pipeline".to_string()
    }

    fn handle(&self, evt: &ActionResponse) {
        todo!()
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&PipelineHandler::id())
    }
}
