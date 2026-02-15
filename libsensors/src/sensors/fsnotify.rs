use super::sensor::{Sensor, SensorEvent};
use crate::sspec::SensorConf;
use async_trait::async_trait;
use filescream::events::{Callback, EventMask, FileScreamEvent};
use filescream::{FileScream, FileScreamConfig};
use serde_json::json;
use std::{fmt, time::Duration};
use tokio::sync::mpsc;

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

impl FsNotifySensor {
    fn arg_str(cfg: &SensorConf, key: &str) -> Option<String> {
        cfg.args().get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn arg_u64(cfg: &SensorConf, key: &str) -> Option<u64> {
        cfg.args().get(key).and_then(|v| v.as_i64()).map(|i| i as u64)
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
        // required args
        let Some(path) = Self::arg_str(&self.cfg, "path") else {
            log::warn!("fsnotify '{}' missing args.path; not starting", self.sid);
            return;
        };

        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3));

        log::info!("fsnotify '{}' watching '{}' with pulse {:?} and opts {:?}", self.sid, path, pulse, self.cfg.opts());

        let mut fs = FileScream::new(Some(FileScreamConfig::default().pulse(pulse)));
        fs.watch(&path);

        // EventMask
        let mut mask = EventMask::empty();
        for o in self.cfg.opts() {
            match o.as_str() {
                "created" => mask |= EventMask::CREATED,
                "changed" => mask |= EventMask::CHANGED,
                "deleted" => mask |= EventMask::REMOVED,
                _ => {
                    log::warn!("fsnotify '{}' unknown opt '{}'", self.sid, o);
                }
            }
        }

        let cb = Callback::new(mask).on(|ev| async move {
            match ev {
                FileScreamEvent::Created { path } => Some(json!({"action":"created","file":path.to_string_lossy()})),
                FileScreamEvent::Changed { path } => Some(json!({"action":"changed","file":path.to_string_lossy()})),
                FileScreamEvent::Removed { path } => Some(json!({"action":"deleted","file":path.to_string_lossy()})),
            }
        });
        fs.add_callback(cb);

        // Channel to receive callback JSON
        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
        fs.set_callback_channel(tx);

        tokio::spawn(fs.run());

        // Forward results
        while let Some(r) = rx.recv().await {
            let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
            let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("unknown");
            let eid = format!("{}|{}|{}@{}|{}", self.sid, FsNotifySensor::id(), action, file, 0);

            (emit)(json!({
                "eid": eid,
                "sensor": self.sid,
                "listener": FsNotifySensor::id(),
                "data": r,
            }));
        }
    }
}
