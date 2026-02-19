use crate::sensors;
use crate::sspec::SensorSpec;
use colored::Colorize;
use libsysinspect::intp::actproc::response::ActionResponse;
use libsysinspect::reactor::evtproc::EventProcessor;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct SensorService {
    spec: SensorSpec,
    reactor: Option<Arc<Mutex<EventProcessor>>>,
}

impl SensorService {
    pub fn new(spec: SensorSpec) -> Self {
        sensors::init_registry();
        Self { spec, reactor: None }
    }

    /// Start all sensors in the service spec, returning a list of JoinHandles for the running tasks.
    pub fn start(&mut self) -> Vec<JoinHandle<()>> {
        let reactor = self.reactor.clone();
        let mut handles = Vec::new();

        for (sid, cfg) in self.spec.items() {
            log::debug!("Starting sensor '{}' with listener '{}'", sid, cfg.listener());

            let Some(sensor) = sensors::init_sensor(cfg.listener(), sid.to_string(), cfg.clone()) else {
                log::error!("Unknown sensor listener '{}' for '{}'", cfg.listener(), sid);
                continue;
            };

            log::info!("Initialized sensor '{}'", format!("{}/{}", sid, cfg.listener()).bright_yellow());

            let sid = sid.to_string();
            let reactor = reactor.clone();

            let emit = move |ev: JsonValue| {
                log::debug!("Registering event from sensor to reactor: {ev}");

                let Some(reactor) = reactor.clone() else {
                    log::warn!("No reactor attached for sensor '{}': {}", sid, ev);
                    return;
                };

                let eid = ev.get("eid").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| sid.clone());
                tokio::spawn(async move {
                    let response = ActionResponse::from_sensor(ev);
                    reactor.lock().await.receiver().register(eid, response);
                });
            };

            handles.push(tokio::spawn(async move {
                sensor.run(&emit).await;
            }));
        }

        handles
    }

    pub fn set_event_processor(&mut self, events: Arc<Mutex<EventProcessor>>) {
        self.reactor = Some(events);
    }
}
