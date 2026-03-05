use crate::bridge::reactor_emitter;
use crate::sensors;
use crate::sspec::SensorSpec;
use colored::Colorize;
use libsysinspect::reactor::evtproc::EventProcessor;
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

            let emit = reactor_emitter(sid.clone(), reactor.clone());
            handles.push(tokio::spawn(async move {
                let sid_for_log = sid.clone();
                let run_res = tokio::task::spawn_blocking(move || {
                    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                        Ok(rt) => rt,
                        Err(e) => {
                            log::error!("Failed to create runtime for sensor '{}': {e}", sid_for_log);
                            return;
                        }
                    };

                    rt.block_on(async move {
                        sensor.run(&emit).await;
                    });
                })
                .await;

                if let Err(e) = run_res {
                    log::error!("Sensor '{}' task panicked or was cancelled: {e}", sid);
                }
            }));
        }

        handles
    }

    pub fn set_event_processor(&mut self, events: Arc<Mutex<EventProcessor>>) {
        self.reactor = Some(events);
    }
}
