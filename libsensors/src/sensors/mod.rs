pub mod fsnotify;
pub mod ifacenotify;
pub mod menotify;
pub mod mountnotify;
pub mod net_health;
pub mod net_route;
pub mod net_throughput;
pub mod net_wifi;
pub mod netnotify;
pub mod net_hostname;
pub mod procnotify;
pub mod sensor;
pub mod socknotify;

#[cfg(test)]
mod ifacenotify_ut;
#[cfg(test)]
mod net_health_ut;
#[cfg(test)]
mod net_hostname_ut;
#[cfg(test)]
mod net_route_ut;
#[cfg(test)]
mod net_throughput_ut;
#[cfg(test)]
mod net_wifi_ut;
mod net_ut;
#[cfg(test)]
mod proc_ut;
#[cfg(test)]
mod socknotify_ut;

use crate::{sensors::sensor::Sensor, sspec::SensorConf};
use dashmap::DashMap;
use lazy_static::lazy_static;

pub type SensorFactory = fn(String, SensorConf) -> Box<dyn Sensor>;
pub type SensorRegistry = DashMap<String, SensorFactory>;

lazy_static! {
    pub static ref REGISTRY: SensorRegistry = DashMap::new();
}

pub fn init_sensor(listener: &str, sid: String, cfg: SensorConf) -> Option<Box<dyn Sensor>> {
    REGISTRY.get(listener).or_else(|| listener.split_once('.').and_then(|(root, _)| REGISTRY.get(root))).map(|f| f(sid, cfg))
}

pub fn init_registry() {
    if !REGISTRY.is_empty() {
        return;
    }

    REGISTRY.insert(fsnotify::FsNotifySensor::id(), |sid: String, cfg: SensorConf| Box::new(fsnotify::FsNotifySensor::new(sid, cfg)));
    REGISTRY.insert(procnotify::ProcessSensor::id(), |sid: String, cfg: SensorConf| Box::new(procnotify::ProcessSensor::new(sid, cfg)));
    REGISTRY.insert(mountnotify::MountSensor::id(), |sid: String, cfg: SensorConf| Box::new(mountnotify::MountSensor::new(sid, cfg)));
    REGISTRY.insert(net_health::NetHealthSensor::id(), |sid: String, cfg: SensorConf| Box::new(net_health::NetHealthSensor::new(sid, cfg)));
    REGISTRY.insert(net_route::NetRouteSensor::id(), |sid: String, cfg: SensorConf| Box::new(net_route::NetRouteSensor::new(sid, cfg)));
    REGISTRY.insert(net_throughput::NetThroughputSensor::id(), |sid: String, cfg: SensorConf| Box::new(net_throughput::NetThroughputSensor::new(sid, cfg)));
    REGISTRY.insert(net_wifi::NetWifiSensor::id(), |sid: String, cfg: SensorConf| Box::new(net_wifi::NetWifiSensor::new(sid, cfg)));
    REGISTRY.insert(netnotify::NetNotifySensor::id(), |sid: String, cfg: SensorConf| Box::new(netnotify::NetNotifySensor::new(sid, cfg)));
    REGISTRY.insert(net_hostname::NetHostnameSensor::id(), |sid: String, cfg: SensorConf| Box::new(net_hostname::NetHostnameSensor::new(sid, cfg)));
    REGISTRY.insert(ifacenotify::IfaceSensor::id(), |sid: String, cfg: SensorConf| Box::new(ifacenotify::IfaceSensor::new(sid, cfg)));
    REGISTRY.insert(menotify::MeNotifySensor::id(), |sid: String, cfg: SensorConf| Box::new(menotify::MeNotifySensor::new(sid, cfg)));
    REGISTRY.insert(socknotify::SockTraySensor::id(), |sid: String, cfg: SensorConf| Box::new(socknotify::SockTraySensor::new(sid, cfg)));
}
