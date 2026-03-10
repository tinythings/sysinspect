use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use omnitrace_core::callbacks::{Callback, CallbackHub};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use xmount::events::{XMountEvent, XMountMask};
use xmount::{XMount, XMountConfig};

pub struct MountSensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for MountSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MountSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl MountSensor {
    pub(crate) fn build_mask(&self) -> XMountMask {
        let mut mask = XMountMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= XMountMask::MOUNTED | XMountMask::UNMOUNTED | XMountMask::CHANGED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "mounted" => mask |= XMountMask::MOUNTED,
                    "unmounted" => mask |= XMountMask::UNMOUNTED,
                    "changed" => mask |= XMountMask::CHANGED,
                    _ => log::warn!("mountnotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", MountSensor::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }
}

#[async_trait]
impl Sensor for MountSensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "mountnotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        let Some(mpoints) = self.cfg.arg_str_array("mountpoints") else {
            log::warn!(
                "[{}] '{}' missing/invalid args.mountpoints (expected array of strings); not starting",
                MountSensor::id().bright_magenta(),
                self.sid
            );
            return;
        };

        let locked = self.cfg.arg_bool("locked").unwrap_or(false);
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(1));

        let mut x = XMount::new(XMountConfig::default().pulse(pulse));
        for mp in &mpoints {
            x.add(mp);
            log::info!("[{}] '{}' watching '{}' with pulse {:?}", MountSensor::id().bright_magenta(), self.sid, mp, pulse);
        }

        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let lstid = self.listener_id_with_tag();

        let mut hub = CallbackHub::<XMountEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: self.build_mask().bits(), sid: self.sid.clone(), lstid, locked });
        let hub = Arc::new(hub);

        let (ctx, _handle) = omnitrace_core::sensor::SensorCtx::new(hub);

        let sid = self.sid.clone();
        tokio::spawn(async move {
            // x.run returns io::Result, just log errors
            if let Err(e) = x.run(ctx).await {
                log::error!("[{}] '{}' error running XMount: {:?}", MountSensor::id().bright_magenta(), sid, e);
            }
        });

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
impl Callback<XMountEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &XMountEvent) -> Option<serde_json::Value> {
        let r = match ev {
            XMountEvent::Mounted { target, info } => json!({
                "action": "mounted",
                "target": target.to_string_lossy(),
                "source": info.source,
                "fstype": info.fstype,
                "opts": info.mount_opts,
            }),
            XMountEvent::Unmounted { target, last } => json!({
                "action": "unmounted",
                "target": target.to_string_lossy(),
                "source": last.source,
                "fstype": last.fstype,
                "opts": last.mount_opts,
            }),
            XMountEvent::Changed { target, old, new } => json!({
                "action": "changed",
                "target": target.to_string_lossy(),
                "old": { "source": old.source, "fstype": old.fstype, "opts": old.mount_opts },
                "new": { "source": new.source, "fstype": new.fstype, "opts": new.mount_opts },
            }),
        };

        let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
        let tgt = r.get("target").and_then(|v| v.as_str()).unwrap_or("unknown");
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, tgt, 0);

        if self.locked && !libcommon::eidhub::get_eidhub().add("mountnotify", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "mountnotify",
            "data": r,
        }))
    }
}
