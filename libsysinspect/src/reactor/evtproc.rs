use super::{callback::EventProcessorCallback, handlers::evthandler::EventHandler, receiver::Receiver};
use crate::{
    intp::conf::Config,
    mdescr::telemetry::TelemetrySpec,
    reactor::handlers::{self},
};

pub struct EventProcessor<'a> {
    receiver: Receiver,
    cfg: Option<&'a Config>,
    handlers: Vec<Box<dyn EventHandler>>,
    action_callbacks: Vec<Box<dyn EventProcessorCallback>>,
    model_callbacks: Vec<Box<dyn EventProcessorCallback>>,
    telemetry_cfg: Option<TelemetrySpec>,
}

impl<'a> EventProcessor<'a> {
    pub fn new() -> Self {
        EventProcessor {
            receiver: Receiver::default(),
            cfg: None,
            handlers: Vec::default(),
            action_callbacks: Vec::default(),
            model_callbacks: Vec::default(),
            telemetry_cfg: None,
        }
    }

    /// Setup event processor from the given configuration
    fn setup(mut self, telemetry_config: Option<TelemetrySpec>) -> Self {
        if self.cfg.is_none() {
            return self;
        }

        self.telemetry_cfg = telemetry_config;

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
    pub fn add_action_callback(&mut self, mut c: Box<dyn EventProcessorCallback>) {
        c.set_telemetry_config(self.telemetry_cfg.clone());
        self.action_callbacks.push(c);
    }

    pub fn add_model_callback(&mut self, mut c: Box<dyn EventProcessorCallback>) {
        c.set_telemetry_config(self.telemetry_cfg.clone());
        self.model_callbacks.push(c);
    }

    /// Get actions receiver
    pub fn receiver(&mut self) -> &mut Receiver {
        &mut self.receiver
    }

    /// Set the configuration of a model
    pub fn set_config(mut self, cfg: &'a Config, tcfg: Option<TelemetrySpec>) -> Self {
        self.cfg = Some(cfg);
        self.setup(tcfg)
    }

    /// Process all handlers
    pub async fn process(&mut self) {
        for ar in self.receiver.get_all() {
            // For each action handle events
            for h in &self.handlers {
                h.handle(&ar);
            }
            // Each action response sent via callback
            for ac in &mut self.action_callbacks {
                _ = ac.on_action_response(ar.clone()).await;
            }
        }

        // Call model callbacks for the last action response (it is usually only passes the minion reference)
        if let Some(ar) = self.receiver.get_last() {
            for cb in &mut self.model_callbacks {
                _ = cb.on_action_response(ar.clone()).await;
            }
        }
    }
}

impl Default for EventProcessor<'_> {
    fn default() -> Self {
        Self::new()
    }
}
