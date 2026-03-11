use crate::{
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use libmenotify::MeNotifyRuntime;
use std::fmt;

pub struct MeNotifySensor {
    runtime: MeNotifyRuntime,
}

impl fmt::Debug for MeNotifySensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeNotifySensor")
            .field("sid", &self.runtime.sid())
            .field("listener", &self.runtime.listener())
            .finish()
    }
}

#[async_trait]
impl Sensor for MeNotifySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self {
            runtime: MeNotifyRuntime::new(id, cfg.listener().to_string()),
        }
    }

    fn id() -> String {
        "menotify".to_string()
    }

    async fn run(&self, _emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        self.runtime.run_stub();
    }
}
