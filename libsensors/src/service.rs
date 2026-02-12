use crate::sensors;
use crate::sspec::SensorSpec;
use serde_json::Value as JsonValue;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct SensorService {
    spec: SensorSpec,
}

impl SensorService {
    pub fn new(spec: SensorSpec) -> Self {
        sensors::init_registry();
        Self { spec }
    }

    /// Start all sensors in the service spec, returning a list of JoinHandles for the running tasks.
    pub fn start(&mut self) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::new();

        for (sid, cfg) in self.spec.items() {
            log::info!("Starting sensor '{}' with listener '{}'", sid, cfg.listener());
            let Some(sensor) = sensors::init_sensor(cfg.listener(), sid.to_string(), cfg.clone()) else {
                log::warn!("Unknown sensor listener '{}' for '{}'", cfg.listener(), sid);
                continue;
            };

            let sid = sid.to_string();

            // emit: for now just println
            let emit = move |ev: JsonValue| {
                println!("SENSOR {} -> {}", sid, ev);
            };

            handles.push(tokio::spawn(async move {
                sensor.run(&emit).await;
            }));
        }

        handles
    }
}
