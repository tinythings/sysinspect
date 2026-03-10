use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use iface::events::{IfaceEvent, IfaceMask};
use iface::{Iface, IfaceConfig};
use omnitrace_core::callbacks::Callback;
use serde_json::json;
use std::collections::HashMap;
use std::{fmt, time::Duration};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub struct IfaceSensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for IfaceSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IfaceSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl IfaceSensor {
    pub(crate) fn build_mask(&self) -> IfaceMask {
        let mut mask = IfaceMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= IfaceMask::IFACE_ADDED
                | IfaceMask::IFACE_REMOVED
                | IfaceMask::LINK_UP
                | IfaceMask::LINK_DOWN
                | IfaceMask::ADDR_ADDED
                | IfaceMask::ADDR_REMOVED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "iface-added" => mask |= IfaceMask::IFACE_ADDED,
                    "iface-removed" => mask |= IfaceMask::IFACE_REMOVED,
                    "link-up" => mask |= IfaceMask::LINK_UP,
                    "link-down" => mask |= IfaceMask::LINK_DOWN,
                    "addr-added" => mask |= IfaceMask::ADDR_ADDED,
                    "addr-removed" => mask |= IfaceMask::ADDR_REMOVED,
                    _ => log::warn!("ifacenotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }
}

#[async_trait]
impl Sensor for IfaceSensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "ifacenotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_millis(250));
        let locked = self.cfg.arg_bool("locked").unwrap_or(false);

        log::info!("[{}] '{}' started with poll timeout {:?} and opts {:?}", Self::id().bright_magenta(), self.sid, pulse, self.cfg.opts());

        let sensor = Iface::new(Some(IfaceConfig::default().poll_timeout(pulse)));
        let mask = self.build_mask();
        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let lstid = self.listener_id_with_tag();

        let mut hub = omnitrace_core::callbacks::CallbackHub::<IfaceEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: mask.bits(), sid: self.sid.clone(), lstid, locked, baseline_link_state: Mutex::new(HashMap::new()) });
        let hub = std::sync::Arc::new(hub);

        let (ctx, _handle) = omnitrace_core::sensor::SensorCtx::new(hub);
        tokio::spawn(sensor.run(ctx));

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
    // Suppress first observed link state per ifindex; backend reports current
    // state on discovery and that is not a transition signal.
    baseline_link_state: Mutex<HashMap<u32, bool>>,
}

#[async_trait::async_trait]
impl Callback<IfaceEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &IfaceEvent) -> Option<serde_json::Value> {
        match ev {
            IfaceEvent::IfaceRemoved { ifindex, .. } => {
                self.baseline_link_state.lock().await.remove(ifindex);
            }
            IfaceEvent::LinkUp { ifindex, .. } => {
                let mut guard = self.baseline_link_state.lock().await;
                if guard.insert(*ifindex, true).is_none() {
                    log::debug!("[ifacenotify] '{}' suppressing baseline link-up for ifindex {}", self.sid, ifindex);
                    return None;
                }
            }
            IfaceEvent::LinkDown { ifindex, .. } => {
                let mut guard = self.baseline_link_state.lock().await;
                if guard.insert(*ifindex, false).is_none() {
                    log::debug!("[ifacenotify] '{}' suppressing baseline link-down for ifindex {}", self.sid, ifindex);
                    return None;
                }
            }
            _ => {}
        }

        let r = match ev {
            IfaceEvent::IfaceAdded { ifindex, ifname } => json!({"action":"iface-added","ifindex":ifindex,"ifname":ifname}),
            IfaceEvent::IfaceRemoved { ifindex, ifname } => json!({"action":"iface-removed","ifindex":ifindex,"ifname":ifname}),
            IfaceEvent::LinkUp { ifindex, ifname } => json!({"action":"link-up","ifindex":ifindex,"ifname":ifname}),
            IfaceEvent::LinkDown { ifindex, ifname } => json!({"action":"link-down","ifindex":ifindex,"ifname":ifname}),
            IfaceEvent::AddrAdded { ifindex, ifname } => json!({"action":"addr-added","ifindex":ifindex,"ifname":ifname}),
            IfaceEvent::AddrRemoved { ifindex, ifname } => json!({"action":"addr-removed","ifindex":ifindex,"ifname":ifname}),
        };

        let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
        let ifname = r.get("ifname").and_then(|v| v.as_str()).unwrap_or("unknown");
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, ifname, 0);

        if self.locked && !libcommon::eidhub::get_eidhub().add("ifacenotify", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "ifacenotify",
            "data": r,
        }))
    }
}
