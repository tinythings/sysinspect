pub mod fsnotify;
pub mod sensor;

use crate::{sensors::sensor::Sensor, sspec::SensorConf};
use dashmap::DashMap;
use lazy_static::lazy_static;

pub type SensorFactory = fn(String, SensorConf) -> Box<dyn Sensor>;
pub type SensorRegistry = DashMap<String, SensorFactory>;

lazy_static! {
    pub static ref REGISTRY: SensorRegistry = DashMap::new();
}

pub fn init_sensor(listener: &str, sid: String, cfg: SensorConf) -> Option<Box<dyn Sensor>> {
    REGISTRY.get(listener).map(|f| f(sid, cfg))
}

pub fn init_registry() {
    if !REGISTRY.is_empty() {
        return;
    }

    REGISTRY.insert(fsnotify::FsNotifySensor::id(), |sid: String, cfg: SensorConf| Box::new(fsnotify::FsNotifySensor::new(sid, cfg)));
}
