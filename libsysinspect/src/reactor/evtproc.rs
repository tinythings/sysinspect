use super::{callback::EventProcessorCallback, handlers::evthandler::EventHandler, receiver::Receiver};
use crate::{
    intp::conf::Config,
    reactor::handlers::{self},
};

pub struct EventProcessor<'a> {
    receiver: Receiver,
    cfg: Option<&'a Config>,
    handlers: Vec<Box<dyn EventHandler>>,
    callbacks: Vec<Box<dyn EventProcessorCallback>>,
}

impl<'a> EventProcessor<'a> {
    pub fn new() -> Self {
        EventProcessor { receiver: Receiver::default(), cfg: None, handlers: Vec::default(), callbacks: Vec::default() }
    }

    /// Setup event processor from the given configuration
    fn setup(mut self) -> Self {
        if self.cfg.is_none() {
            return self;
        }

        let cfg = self.cfg.unwrap();
        for evt_id in cfg.get_event_ids() {
            let evt_cfg = cfg.get_event(&evt_id).unwrap();
            for handler_id in evt_cfg.get_bound_handlers() {
                if let Some(handler) =
                    handlers::registry::init_handler(handler_id.to_string(), evt_id.to_string(), evt_cfg.to_owned())
                {
                    self.handlers.push(handler);
                    log::debug!("Registered handler: {handler_id} on {evt_id}")
                } else {
                    log::error!("Unknown handler: {handler_id}");
                }
            }
        }

        self
    }

    /// Add a callback
    pub fn add_callback(&mut self, c: Box<dyn EventProcessorCallback>) {
        self.callbacks.push(c);
    }

    /// Get actions receiver
    pub fn receiver(&mut self) -> &mut Receiver {
        &mut self.receiver
    }

    /// Set the configuration of a model
    pub fn set_config(mut self, cfg: &'a Config) -> Self {
        self.cfg = Some(cfg);
        self.setup()
    }

    /// Process all handlers
    pub fn process(&mut self) {
        for ar in self.receiver.get_all() {
            // For each action handle events
            for h in &self.handlers {
                h.handle(&ar);
                for c in &mut self.callbacks {
                    c.on_action_response(ar.clone());
                }
            }
        }
    }
}

impl Default for EventProcessor<'_> {
    fn default() -> Self {
        Self::new()
    }
}
