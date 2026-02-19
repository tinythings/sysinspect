use std::sync::Arc;

use super::{callback::EventProcessorCallback, handlers::evthandler::EventHandler, receiver::Receiver};
use crate::{
    intp::conf::EventsConfig,
    mdescr::telemetry::TelemetrySpec,
    reactor::handlers::{self},
};

#[derive(Debug)]
pub struct EventProcessor {
    receiver: Receiver,
    cfg: Option<Arc<EventsConfig>>,
    handlers: Vec<Box<dyn EventHandler>>,
    action_callbacks: Vec<Box<dyn EventProcessorCallback>>,
    model_callbacks: Vec<Box<dyn EventProcessorCallback>>,
    telemetry_cfg: Option<TelemetrySpec>,
}

impl EventProcessor {
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

        let cfg = self.cfg.as_ref().unwrap();
        for evt_id in cfg.get_event_ids() {
            let evt_cfg = cfg.get_event(&evt_id).unwrap();
            for handler_id in evt_cfg.get_bound_handlers() {
                if let Some(handler) = handlers::registry::init_handler(handler_id.to_string(), evt_id.to_string(), evt_cfg.to_owned()) {
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
    pub fn set_config(mut self, cfg: Arc<EventsConfig>, tcfg: Option<TelemetrySpec>) -> Self {
        self.cfg = Some(cfg);
        self.setup(tcfg)
    }

    /// Process all handlers
    pub async fn process(&mut self, drain: bool) {
        let batch = if drain { self.receiver.drain_all() } else { self.receiver.get_all() };
        let last = batch.last().cloned();

        for ar in batch {
            for h in &self.handlers {
                h.handle(&ar);
            }
            for ac in &mut self.action_callbacks {
                _ = ac.on_action_response(ar.clone()).await;
            }
        }

        if let Some(ar) = last {
            for cb in &mut self.model_callbacks {
                _ = cb.on_action_response(ar.clone()).await;
            }
        }
    }
}

impl Default for EventProcessor {
    fn default() -> Self {
        Self::new()
    }
}
