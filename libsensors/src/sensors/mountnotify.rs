use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use serde_json::json;
use std::{fmt, time::Duration};

#[cfg(target_os = "freebsd")]
use std::{collections::HashMap, process::Command};
#[cfg(target_os = "freebsd")]
use tokio::time;

#[cfg(not(target_os = "freebsd"))]
use omnitrace_core::callbacks::{Callback, CallbackHub};
#[cfg(not(target_os = "freebsd"))]
use std::sync::Arc;
#[cfg(not(target_os = "freebsd"))]
use tokio::sync::mpsc;
#[cfg(not(target_os = "freebsd"))]
use xmount::events::XMountEvent;
#[cfg(not(target_os = "freebsd"))]
pub(crate) use xmount::events::XMountMask;
#[cfg(not(target_os = "freebsd"))]
use xmount::{XMount, XMountConfig};

#[cfg(target_os = "freebsd")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct XMountMask(u8);

#[cfg(target_os = "freebsd")]
impl XMountMask {
    pub(crate) const MOUNTED: Self = Self(0b001);
    pub(crate) const UNMOUNTED: Self = Self(0b010);
    pub(crate) const CHANGED: Self = Self(0b100);

    pub(crate) fn empty() -> Self {
        Self(0)
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(target_os = "freebsd")]
impl std::ops::BitOrAssign for XMountMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(target_os = "freebsd")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MountInfo {
    source: String,
    fstype: String,
    opts: Vec<String>,
}

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
            mask |= XMountMask::MOUNTED;
            mask |= XMountMask::UNMOUNTED;
            mask |= XMountMask::CHANGED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "mounted" => mask |= XMountMask::MOUNTED,
                    "unmounted" => mask |= XMountMask::UNMOUNTED,
                    "changed" => mask |= XMountMask::CHANGED,
                    _ => log::warn!("sys.mount '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", MountSensor::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }

    #[cfg(target_os = "freebsd")]
    fn freebsd_snapshot(&self) -> HashMap<String, MountInfo> {
        Command::new("mount")
            .arg("-p")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .map(|stdout| {
                let wanted = self.cfg.arg_str_array("mountpoints").unwrap_or_default();
                stdout
                    .lines()
                    .filter_map(|line| {
                        let fields = line.split_whitespace().collect::<Vec<_>>();
                        (fields.len() >= 4).then(|| {
                            (
                                fields[1].to_string(),
                                MountInfo {
                                    source: fields[0].to_string(),
                                    fstype: fields[2].to_string(),
                                    opts: fields[3].split(',').map(|opt| opt.to_string()).collect(),
                                },
                            )
                        })
                    })
                    .filter(|(target, _)| wanted.iter().any(|want| want == target))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "freebsd")]
    async fn emit_freebsd(
        &self, emit: &(dyn Fn(SensorEvent) + Send + Sync), locked: bool, action: &str, target: &str, old: Option<&MountInfo>, new: Option<&MountInfo>,
    ) {
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.listener_id_with_tag(), action, target, 0);
        if locked && !libcommon::eidhub::get_eidhub().add("sys.mount", &eid).await {
            return;
        }

        (emit)(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "sys.mount",
            "data": match action {
                "mounted" => json!({
                    "action": "mounted",
                    "target": target,
                    "source": new.map(|item| item.source.clone()).unwrap_or_default(),
                    "fstype": new.map(|item| item.fstype.clone()).unwrap_or_default(),
                    "opts": new.map(|item| item.opts.clone()).unwrap_or_default(),
                }),
                "unmounted" => json!({
                    "action": "unmounted",
                    "target": target,
                    "source": old.map(|item| item.source.clone()).unwrap_or_default(),
                    "fstype": old.map(|item| item.fstype.clone()).unwrap_or_default(),
                    "opts": old.map(|item| item.opts.clone()).unwrap_or_default(),
                }),
                _ => json!({
                    "action": "changed",
                    "target": target,
                    "old": {
                        "source": old.map(|item| item.source.clone()).unwrap_or_default(),
                        "fstype": old.map(|item| item.fstype.clone()).unwrap_or_default(),
                        "opts": old.map(|item| item.opts.clone()).unwrap_or_default(),
                    },
                    "new": {
                        "source": new.map(|item| item.source.clone()).unwrap_or_default(),
                        "fstype": new.map(|item| item.fstype.clone()).unwrap_or_default(),
                        "opts": new.map(|item| item.opts.clone()).unwrap_or_default(),
                    },
                }),
            },
        }));
    }
}

#[async_trait]
impl Sensor for MountSensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "sys.mount".to_string()
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

        #[cfg(not(target_os = "freebsd"))]
        {
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
                if let Err(e) = x.run(ctx).await {
                    log::error!("[{}] '{}' error running XMount: {:?}", MountSensor::id().bright_magenta(), sid, e);
                }
            });

            while let Some(v) = rx.recv().await {
                (emit)(v);
            }
        }

        #[cfg(target_os = "freebsd")]
        {
            let locked = self.cfg.arg_bool("locked").unwrap_or(false);
            let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(1));
            let mask = self.build_mask();
            let mut seen = self.freebsd_snapshot();
            let mut tick = time::interval(pulse);

            for mp in &mpoints {
                log::info!("[{}] '{}' watching '{}' with pulse {:?} via mount -p", MountSensor::id().bright_magenta(), self.sid, mp, pulse);
            }

            loop {
                tick.tick().await;
                let current = self.freebsd_snapshot();

                if mask.contains(XMountMask::MOUNTED) {
                    for (target, item) in current.iter().filter(|(target, _)| !seen.contains_key(*target)) {
                        self.emit_freebsd(emit, locked, "mounted", target, None, Some(item)).await;
                    }
                }

                if mask.contains(XMountMask::UNMOUNTED) {
                    for (target, item) in seen.iter().filter(|(target, _)| !current.contains_key(*target)) {
                        self.emit_freebsd(emit, locked, "unmounted", target, Some(item), None).await;
                    }
                }

                if mask.contains(XMountMask::CHANGED) {
                    for (target, item, old) in current.iter().filter_map(|(target, item)| seen.get(target).map(|old| (target, item, old))) {
                        if item != old {
                            self.emit_freebsd(emit, locked, "changed", target, Some(old), Some(item)).await;
                        }
                    }
                }

                seen = current;
            }
        }
    }
}

#[cfg(not(target_os = "freebsd"))]
struct BridgeCb {
    mask: u64,
    sid: String,
    lstid: String,
    locked: bool,
}

#[cfg(not(target_os = "freebsd"))]
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

        if self.locked && !libcommon::eidhub::get_eidhub().add("sys.mount", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "sys.mount",
            "data": r,
        }))
    }
}
