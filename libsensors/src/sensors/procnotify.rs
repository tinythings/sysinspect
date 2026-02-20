use crate::{
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use procdog::{
    ProcDog, ProcDogConfig,
    events::{Callback, EventMask, ProcDogEvent},
};
use serde_json::json;
use std::{fmt, time::Duration};
use tokio::sync::mpsc;

pub struct ProcessSensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for ProcessSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl ProcessSensor {
    fn arg_str(cfg: &SensorConf, key: &str) -> Option<String> {
        cfg.args().get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn arg_u64(cfg: &SensorConf, key: &str) -> Option<u64> {
        cfg.args().get(key).and_then(|v| v.as_i64()).map(|i| i as u64)
    }
}

#[async_trait]
impl Sensor for ProcessSensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    /// Return the listener name.
    fn id() -> String {
        "procnotify".to_string()
    }

    /// Run the sensor.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        let Some(process) = Self::arg_str(&self.cfg, "process") else {
            log::warn!("procnotify '{}' missing args.process; not starting", self.sid);
            return;
        };
        if process.trim().is_empty() {
            log::warn!("procnotify '{}' empty args.process; not starting", self.sid);
            return;
        };

        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3));
        log::info!("procnotify '{}' watching '{}' with pulse {:?} and opts {:?}", self.sid, process, pulse, self.cfg.opts());

        let mut dog = ProcDog::new(Some(ProcDogConfig::default().interval(pulse)));

        // Choose backends
        #[cfg(target_os = "linux")]
        dog.set_backend(procdog::backends::linuxps::LinuxPsBackend);

        #[cfg(target_os = "netbsd")]
        dog.set_backend(procdog::backends::netbsd_sysctl::NetBsdSysctlBackend);

        #[cfg(all(not(target_os = "linux"), not(target_os = "netbsd")))]
        dog.set_backend(procdog::backends::stps::PsBackend);

        dog.watch(&process);

        let mut mask = EventMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= EventMask::APPEARED | EventMask::DISAPPEARED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "appeared" => mask |= EventMask::APPEARED,
                    "disappeared" => mask |= EventMask::DISAPPEARED,
                    _ => log::warn!("procnotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }

        let cb = Callback::new(mask).on(|ev| async move {
            match ev {
                ProcDogEvent::Appeared { name, pid } => Some(json!({
                    "action": "appeared",
                    "process": name,
                    "pid": pid,
                })),
                ProcDogEvent::Disappeared { name, pid } => Some(json!({
                    "action": "disappeared",
                    "process": name,
                    "pid": pid,
                })),

                // For later...
                #[allow(unreachable_patterns)]
                other => Some(json!({
                    "action": "unknown",
                    "event": format!("{:?}", other),
                })),
            }
        });

        dog.add_callback(cb);

        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
        dog.set_callback_channel(tx);

        tokio::spawn(dog.run());

        while let Some(r) = rx.recv().await {
            let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
            let pname = r.get("process").and_then(|v| v.as_str()).unwrap_or(&process);
            let lstid = format!("{}{}{}", ProcessSensor::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""));
            let eid = format!("{}|{}|{}@{}|{}", self.sid, lstid, action, pname, 0);

            (emit)(json!({
                "eid": eid,
                "sensor": self.sid,
                "listener": ProcessSensor::id(),
                "data": r,
            }));
        }
    }
}
