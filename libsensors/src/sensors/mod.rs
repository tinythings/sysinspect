pub mod fsnotify;
#[cfg(not(target_os = "freebsd"))]
pub mod ifacenotify;
pub mod menotify;
#[cfg(not(target_os = "freebsd"))]
pub mod mountnotify;
pub mod net_health;
pub mod net_hostname;
pub mod net_route;
pub mod net_throughput;
pub mod net_wifi;
pub mod netnotify;
pub mod procnotify;
pub mod sensor;
#[cfg(not(target_os = "freebsd"))]
pub mod socknotify;

#[cfg(test)]
#[cfg(not(target_os = "freebsd"))]
mod ifacenotify_ut;
#[cfg(test)]
mod net_health_ut;
#[cfg(test)]
mod net_hostname_ut;
#[cfg(test)]
mod net_route_ut;
#[cfg(test)]
mod net_throughput_ut;
mod net_ut;
#[cfg(test)]
mod net_wifi_ut;
#[cfg(test)]
mod proc_ut;
#[cfg(test)]
#[cfg(not(target_os = "freebsd"))]
mod socknotify_ut;

use crate::{sensors::sensor::Sensor, sspec::SensorConf};
use dashmap::DashMap;
use lazy_static::lazy_static;
use std::path::PathBuf;

/// Runtime context passed to sensor constructors.
#[derive(Debug, Clone, Default)]
pub struct SensorCtx {
    sharelib_root: Option<PathBuf>,
}

impl SensorCtx {
    /// Returns a context with an explicit Sysinspect share library root.
    pub fn with_sharelib_root(mut self, root: PathBuf) -> Self {
        self.sharelib_root = Some(root);
        self
    }

    /// Returns the configured Sysinspect share library root, if any.
    pub fn sharelib_root(&self) -> Option<&std::path::Path> {
        self.sharelib_root.as_deref()
    }
}

pub type SensorFactory = fn(String, SensorConf, SensorCtx) -> Box<dyn Sensor>;
pub type SensorRegistry = DashMap<String, SensorFactory>;

lazy_static! {
    pub static ref REGISTRY: SensorRegistry = DashMap::new();
}

/// Initialises one sensor instance for the given listener id and runtime context.
pub fn init_sensor(listener: &str, sid: String, cfg: SensorConf, ctx: SensorCtx) -> Option<Box<dyn Sensor>> {
    REGISTRY.get(listener).or_else(|| listener.split_once('.').and_then(|(root, _)| REGISTRY.get(root))).map(|f| f(sid, cfg, ctx))
}

/// Registers all built-in sensor factories.
pub fn init_registry() {
    if !REGISTRY.is_empty() {
        return;
    }

    REGISTRY
        .insert(fsnotify::FsNotifySensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(fsnotify::FsNotifySensor::new(sid, cfg)));
    REGISTRY
        .insert(procnotify::ProcessSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(procnotify::ProcessSensor::new(sid, cfg)));
    #[cfg(not(target_os = "freebsd"))]
    REGISTRY
        .insert(mountnotify::MountSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(mountnotify::MountSensor::new(sid, cfg)));
    REGISTRY.insert(net_health::NetHealthSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| {
        Box::new(net_health::NetHealthSensor::new(sid, cfg))
    });
    REGISTRY
        .insert(net_route::NetRouteSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(net_route::NetRouteSensor::new(sid, cfg)));
    REGISTRY.insert(net_throughput::NetThroughputSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| {
        Box::new(net_throughput::NetThroughputSensor::new(sid, cfg))
    });
    REGISTRY.insert(net_wifi::NetWifiSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(net_wifi::NetWifiSensor::new(sid, cfg)));
    REGISTRY.insert(netnotify::NetNotifySensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| {
        Box::new(netnotify::NetNotifySensor::new(sid, cfg))
    });
    REGISTRY.insert(net_hostname::NetHostnameSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| {
        Box::new(net_hostname::NetHostnameSensor::new(sid, cfg))
    });
    #[cfg(not(target_os = "freebsd"))]
    REGISTRY
        .insert(ifacenotify::IfaceSensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| Box::new(ifacenotify::IfaceSensor::new(sid, cfg)));
    REGISTRY.insert(menotify::MeNotifySensor::id(), |sid: String, cfg: SensorConf, ctx: SensorCtx| {
        Box::new(menotify::MeNotifySensor::with_ctx(sid, cfg, ctx))
    });
    #[cfg(not(target_os = "freebsd"))]
    REGISTRY.insert(socknotify::SockTraySensor::id(), |sid: String, cfg: SensorConf, _ctx: SensorCtx| {
        Box::new(socknotify::SockTraySensor::new(sid, cfg))
    });
}
