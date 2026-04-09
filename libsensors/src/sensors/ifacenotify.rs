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
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    process::Command,
};
#[cfg(target_os = "freebsd")]
use tokio::time;

#[cfg(not(target_os = "freebsd"))]
use iface::events::IfaceEvent;
#[cfg(not(target_os = "freebsd"))]
pub(crate) use iface::events::IfaceMask;
#[cfg(not(target_os = "freebsd"))]
use iface::{Iface, IfaceConfig};
#[cfg(not(target_os = "freebsd"))]
use omnitrace_core::callbacks::Callback;
#[cfg(not(target_os = "freebsd"))]
use std::collections::HashMap;
#[cfg(not(target_os = "freebsd"))]
use tokio::sync::Mutex;
#[cfg(not(target_os = "freebsd"))]
use tokio::sync::mpsc;

#[cfg(target_os = "freebsd")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IfaceMask(u8);

#[cfg(target_os = "freebsd")]
impl IfaceMask {
    pub(crate) const IFACE_ADDED: Self = Self(0b000001);
    pub(crate) const IFACE_REMOVED: Self = Self(0b000010);
    pub(crate) const LINK_UP: Self = Self(0b000100);
    pub(crate) const LINK_DOWN: Self = Self(0b001000);
    pub(crate) const ADDR_ADDED: Self = Self(0b010000);
    pub(crate) const ADDR_REMOVED: Self = Self(0b100000);

    pub(crate) fn empty() -> Self {
        Self(0)
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(target_os = "freebsd")]
impl std::ops::BitOrAssign for IfaceMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

pub struct IfaceSensor {
    sid: String,
    cfg: SensorConf,
}

#[cfg(target_os = "freebsd")]
#[derive(Clone, Debug, Default)]
struct IfaceState {
    ifindex: u32,
    link_up: bool,
    addrs: HashSet<String>,
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
            mask |= IfaceMask::IFACE_ADDED;
            mask |= IfaceMask::IFACE_REMOVED;
            mask |= IfaceMask::LINK_UP;
            mask |= IfaceMask::LINK_DOWN;
            mask |= IfaceMask::ADDR_ADDED;
            mask |= IfaceMask::ADDR_REMOVED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "iface-added" => mask |= IfaceMask::IFACE_ADDED,
                    "iface-removed" => mask |= IfaceMask::IFACE_REMOVED,
                    "link-up" => mask |= IfaceMask::LINK_UP,
                    "link-down" => mask |= IfaceMask::LINK_DOWN,
                    "addr-added" => mask |= IfaceMask::ADDR_ADDED,
                    "addr-removed" => mask |= IfaceMask::ADDR_REMOVED,
                    _ => log::warn!("net.iface '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }

    #[cfg(target_os = "freebsd")]
    fn ifindex(ifname: &str) -> u32 {
        CString::new(ifname)
            .ok()
            .map(|ifname| unsafe { libc::if_nametoindex(ifname.as_ptr()) })
            .unwrap_or_default()
    }

    #[cfg(target_os = "freebsd")]
    fn freebsd_snapshot(&self) -> HashMap<String, IfaceState> {
        Command::new("ifconfig")
            .arg("-a")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .map(|stdout| {
                let mut out = HashMap::<String, IfaceState>::new();
                let mut ifname = String::new();

                for line in stdout.lines() {
                    if !line.starts_with('\t') && line.contains(':') {
                        ifname = line.split(':').next().unwrap_or_default().trim().to_string();
                        out.insert(
                            ifname.clone(),
                            IfaceState {
                                ifindex: Self::ifindex(&ifname),
                                link_up: line.contains("<UP") || line.contains(",UP,") || line.contains("status: active"),
                                addrs: HashSet::new(),
                            },
                        );
                    } else if !ifname.is_empty() && let Some(state) = out.get_mut(&ifname) {
                        let line = line.trim();
                        if line.starts_with("status:") {
                            state.link_up = line.eq("status: active");
                        } else if line.starts_with("inet ") || line.starts_with("inet6 ") {
                            if let Some(addr) = line.split_whitespace().nth(1) {
                                state.addrs.insert(addr.split('%').next().unwrap_or(addr).to_string());
                            }
                        }
                    }
                }

                out
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "freebsd")]
    async fn emit_freebsd(
        &self,
        emit: &(dyn Fn(SensorEvent) + Send + Sync),
        locked: bool,
        action: &str,
        ifname: &str,
        ifindex: u32,
        addr: Option<&str>,
    ) {
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.listener_id_with_tag(), action, ifname, 0);
        if locked && !libcommon::eidhub::get_eidhub().add("net.iface", &eid).await {
            return;
        }

        (emit)(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "net.iface",
            "data": {
                "action": action,
                "ifindex": ifindex,
                "ifname": ifname,
                "addr": addr,
            },
        }));
    }
}

#[async_trait]
impl Sensor for IfaceSensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "net.iface".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        #[cfg(not(target_os = "freebsd"))]
        {
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

        #[cfg(target_os = "freebsd")]
        {
            let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_millis(250));
            let locked = self.cfg.arg_bool("locked").unwrap_or(false);
            let mask = self.build_mask();
            let mut seen = self.freebsd_snapshot();
            let mut tick = time::interval(pulse);

            log::info!("[{}] '{}' started with poll timeout {:?} and opts {:?} via ifconfig", Self::id().bright_magenta(), self.sid, pulse, self.cfg.opts());

            loop {
                tick.tick().await;
                let current = self.freebsd_snapshot();

                if mask.contains(IfaceMask::IFACE_ADDED) {
                    for (ifname, state) in current.iter().filter(|(ifname, _)| !seen.contains_key(*ifname)) {
                        self.emit_freebsd(emit, locked, "iface-added", ifname, state.ifindex, None).await;
                    }
                }

                if mask.contains(IfaceMask::IFACE_REMOVED) {
                    for (ifname, state) in seen.iter().filter(|(ifname, _)| !current.contains_key(*ifname)) {
                        self.emit_freebsd(emit, locked, "iface-removed", ifname, state.ifindex, None).await;
                    }
                }

                for (ifname, state, old) in current.iter().filter_map(|(ifname, state)| seen.get(ifname).map(|old| (ifname, state, old))) {
                    if mask.contains(IfaceMask::LINK_UP) && !state.link_up.eq(&old.link_up) && state.link_up {
                        self.emit_freebsd(emit, locked, "link-up", ifname, state.ifindex, None).await;
                    }
                    if mask.contains(IfaceMask::LINK_DOWN) && !state.link_up.eq(&old.link_up) && !state.link_up {
                        self.emit_freebsd(emit, locked, "link-down", ifname, state.ifindex, None).await;
                    }
                    if mask.contains(IfaceMask::ADDR_ADDED) {
                        for addr in state.addrs.iter().filter(|addr| !old.addrs.contains(*addr)) {
                            self.emit_freebsd(emit, locked, "addr-added", ifname, state.ifindex, Some(addr)).await;
                        }
                    }
                    if mask.contains(IfaceMask::ADDR_REMOVED) {
                        for addr in old.addrs.iter().filter(|addr| !state.addrs.contains(*addr)).cloned().collect::<Vec<_>>() {
                            self.emit_freebsd(emit, locked, "addr-removed", ifname, state.ifindex, Some(&addr)).await;
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
    baseline_link_state: Mutex<HashMap<u32, bool>>,
}

#[cfg(not(target_os = "freebsd"))]
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
                    log::debug!("[net.iface] '{}' suppressing baseline link-up for ifindex {}", self.sid, ifindex);
                    return None;
                }
            }
            IfaceEvent::LinkDown { ifindex, .. } => {
                let mut guard = self.baseline_link_state.lock().await;
                if guard.insert(*ifindex, false).is_none() {
                    log::debug!("[net.iface] '{}' suppressing baseline link-down for ifindex {}", self.sid, ifindex);
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

        if self.locked && !libcommon::eidhub::get_eidhub().add("net.iface", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "net.iface",
            "data": r,
        }))
    }
}
