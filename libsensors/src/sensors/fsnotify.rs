use super::sensor::{Sensor, SensorEvent};
use crate::argparse::SensorArgs;
use crate::sspec::SensorConf;
use async_trait::async_trait;
use colored::Colorize;
use filescream::{FileScream, FileScreamConfig, events::FileScreamEvent};
use omnitrace_core::callbacks::Callback;
use std::{fmt, time::Duration};
#[derive(Clone)]
pub struct FsNotifySensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for FsNotifySensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FsNotifySensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

#[async_trait]
impl Sensor for FsNotifySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "fsnotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        // args
        let Some(path) = self.cfg.arg_str("path") else {
            log::warn!("[{}] '{}' missing args.path; not starting", Self::id().bright_magenta(), self.sid);
            return;
        };

        let locked = self.cfg.arg_bool("locked").unwrap_or(false);
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3));

        log::info!("[{}] '{}' watching '{}' with pulse {:?} and opts {:?}", Self::id().bright_magenta(), self.sid, path, pulse, self.cfg.opts());

        // build sensor
        let mut fs = FileScream::new(Some(FileScreamConfig::default().pulse(pulse)));
        fs.watch(&path);

        // build mask
        let mut mask = filescream::events::FileScreamMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= filescream::events::FileScreamMask::CREATED
                | filescream::events::FileScreamMask::CHANGED
                | filescream::events::FileScreamMask::REMOVED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "created" => mask |= filescream::events::FileScreamMask::CREATED,
                    "changed" => mask |= filescream::events::FileScreamMask::CHANGED,
                    "deleted" => mask |= filescream::events::FileScreamMask::REMOVED,
                    _ => log::warn!("fsnotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(0xfff);
        let lstid = format!("{}{}{}", FsNotifySensor::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""));
        let mut hub = omnitrace_core::callbacks::CallbackHub::<FileScreamEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: mask.bits(), sid: self.sid.clone(), lstid, locked });
        let hub = std::sync::Arc::new(hub);

        let (ctx, _handle) = omnitrace_core::sensor::SensorCtx::new(hub);
        tokio::spawn(fs.run(ctx));

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
impl Callback<FileScreamEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &FileScreamEvent) -> Option<serde_json::Value> {
        let r = match ev {
            FileScreamEvent::Created { path } => serde_json::json!({"action":"created","file":path.to_string_lossy()}),
            FileScreamEvent::Changed { path } => serde_json::json!({"action":"changed","file":path.to_string_lossy()}),
            FileScreamEvent::Removed { path } => serde_json::json!({"action":"deleted","file":path.to_string_lossy()}),
        };

        let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
        let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("unknown");
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, file, 0);

        if self.locked && !libcommon::eidhub::get_eidhub().add("fsnotify", &eid).await {
            return None;
        }

        Some(serde_json::json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "fsnotify",
            "data": r,
        }))
    }
}
