use crate::bridge::reactor_emitter;
use crate::sensors;
use crate::sspec::SensorSpec;
use colored::Colorize;
use libsysinspect::reactor::evtproc::EventProcessor;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::{JoinHandle, JoinSet};

pub struct SensorService {
    spec: SensorSpec,
    reactor: Option<Arc<Mutex<EventProcessor>>>,
}

struct AbortOnDropHandle(Option<JoinHandle<()>>);

impl Drop for AbortOnDropHandle {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            h.abort();
        }
    }
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
                sensor.run(&emit).await;
            }));
        }

        handles
    }

    /// Start all sensors under one supervisor task so aborting the returned
    /// handle aborts all running sensor tasks as well.
    ///
    /// # Returns
    ///
    /// Returns a join handle for the supervisor task.
    pub fn spawn(mut self) -> JoinHandle<()> {
        let handles = self.start();
        tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            for h in handles {
                tasks.spawn(async move {
                    let mut h = AbortOnDropHandle(Some(h));
                    if let Some(h) = h.0.take() {
                        let _ = h.await;
                    }
                });
            }

            while tasks.join_next().await.is_some() {}
        })
    }

    pub fn set_event_processor(&mut self, events: Arc<Mutex<EventProcessor>>) {
        self.reactor = Some(events);
    }
}
