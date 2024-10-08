use super::{handlers::evthandler::EventHandler, receiver::Receiver};
use crate::intp::conf::Config;

pub struct EventProcessor<'a> {
    rec: Receiver,
    cfg: Option<&'a Config>,
    handlers: Vec<&'a dyn EventHandler>,
}

impl<'a> EventProcessor<'a> {
    pub fn new(rec: Receiver) -> Self {
        EventProcessor { rec, cfg: None, handlers: Vec::default() }
    }

    /// Set the configuration of a model
    pub fn set_config(mut self, cfg: &'a Config) -> Self {
        self.cfg = Some(cfg);
        self
    }

    /// Add an event handler
    pub fn add_handler(&mut self, handler: &'a dyn EventHandler) {
        self.handlers.push(handler);
    }

    /// Process all handlers
    pub fn process(&self) {
        for ar in self.rec.get_all() {
            // For each action handle events
            for h in &self.handlers {
                h.handle(&ar);
            }
        }
    }
}
