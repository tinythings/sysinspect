use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use omnitrace_core::callbacks::Callback;
use procdog::events::{ProcDogEvent, ProcDogMask};
use procdog::{ProcDog, ProcDogConfig};
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
    pub(crate) fn build_mask(&self) -> ProcDogMask {
        let mut mask = ProcDogMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= ProcDogMask::APPEARED | ProcDogMask::DISAPPEARED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "appeared" => mask |= ProcDogMask::APPEARED,
                    "disappeared" => mask |= ProcDogMask::DISAPPEARED,
                    "missing" => mask |= ProcDogMask::MISSING,
                    _ => log::warn!("procnotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    pub(crate) fn event_to_json(ev: ProcDogEvent) -> serde_json::Value {
        match ev {
            ProcDogEvent::Appeared { name, pid } => json!({
                "action": "appeared",
                "process": name,
                "pid": pid,
            }),
            ProcDogEvent::Disappeared { name, pid } => json!({
                "action": "disappeared",
                "process": name,
                "pid": pid,
            }),
            ProcDogEvent::Missing { name } => json!({
                "action": "missing",
                "process": name,
            }),
            #[allow(unreachable_patterns)]
            other => json!({
                "action": "unknown",
                "event": format!("{:?}", other),
            }),
        }
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", ProcessSensor::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }

    pub fn make_eid(&self, action: &str, pname: &str) -> String {
        let lstid = self.listener_id_with_tag();
        format!("{}|{}|{}@{}|{}", self.sid, lstid, action, pname, 0)
    }

    fn set_backend(dog: &mut ProcDog) {
        #[cfg(target_os = "linux")]
        dog.set_backend(procdog::backends::linuxps::LinuxPsBackend);

        #[cfg(target_os = "netbsd")]
        dog.set_backend(procdog::backends::netbsd_sysctl::NetBsdSysctlBackend);

        #[cfg(all(not(target_os = "linux"), not(target_os = "netbsd")))]
        dog.set_backend(procdog::backends::stps::PsBackend);
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
        let Some(processes) = self.cfg.arg_str_array("process") else {
            log::warn!(
                "[{}] '{}' missing/invalid args.process (expected array of strings); not starting",
                ProcessSensor::id().bright_magenta(),
                self.sid
            );
            return;
        };

        let start_emit = self.cfg.arg_bool("emit-on-start").unwrap_or(false);
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3));
        let locked = self.cfg.arg_bool("locked").unwrap_or(false);

        let mut dog = ProcDog::new(Some(ProcDogConfig::default().interval(pulse).emit_on_start(start_emit)));
        Self::set_backend(&mut dog);

        for p in &processes {
            dog.watch(p);
            log::info!("[{}] '{}' watching '{}' with pulse {:?}", ProcessSensor::id().bright_magenta(), self.sid, p, pulse);
        }

        let mask = self.build_mask();

        // results channel (callback returns JSON envelope)
        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);

        let lstid = self.listener_id_with_tag();

        // hub
        let mut hub = omnitrace_core::callbacks::CallbackHub::<procdog::events::ProcDogEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: mask.bits(), sid: self.sid.clone(), lstid, locked });
        let hub = std::sync::Arc::new(hub);

        let (ctx, _handle) = omnitrace_core::sensor::SensorCtx::new(hub);

        // run sensor + forward callback results
        tokio::spawn(dog.run(ctx));

        while let Some(v) = rx.recv().await {
            (emit)(v);
        }
    }
}

struct BridgeCb {
    mask: u64,
    sid: String,
    lstid: String,
    locked: bool,
}

#[async_trait::async_trait]
impl Callback<ProcDogEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &ProcDogEvent) -> Option<serde_json::Value> {
        let r = match ev {
            ProcDogEvent::Appeared { name, pid } => json!({"action":"appeared","process":name,"pid":pid}),
            ProcDogEvent::Disappeared { name, pid } => json!({"action":"disappeared","process":name,"pid":pid}),
            ProcDogEvent::Missing { name } => json!({"action":"missing","process":name}),
        };

        let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
        let pname = r.get("process").and_then(|v| v.as_str()).unwrap_or("unknown");
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, pname, 0);

        if self.locked && !libcommon::eidhub::get_eidhub().add("procnotify", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "procnotify",
            "data": r,
        }))
    }
}
