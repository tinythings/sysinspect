use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use netpacket::events::{NetNotifyEvent, NetNotifyMask};
use netpacket::{NetNotify, NetNotifyConfig};
use omnitrace_core::callbacks::Callback;
use serde_json::json;
use std::{fmt, time::Duration};
use tokio::sync::mpsc;

pub struct NetNotifySensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for NetNotifySensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetNotifySensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetNotifySensor {
    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }

    fn build_mask(&self) -> NetNotifyMask {
        let mut mask = NetNotifyMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= NetNotifyMask::OPENED | NetNotifyMask::CLOSED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "opened" => mask |= NetNotifyMask::OPENED,
                    "closed" => mask |= NetNotifyMask::CLOSED,
                    _ => log::warn!("netnotify '{}' unknown opt '{}'", self.sid, o),
                }
            }
        }
        mask
    }

    // rule: if pattern has letters or glob -> enable reverse DNS
    fn pattern_needs_dns(p: &str) -> bool {
        p.contains('*') || p.chars().any(|c| c.is_ascii_alphabetic())
    }
}

#[async_trait]
impl Sensor for NetNotifySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "netnotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(1));
        let locked = self.cfg.arg_bool("locked").unwrap_or(false);

        let patterns = self.cfg.arg_str_array("patterns").unwrap_or_default();
        let ignores = self.cfg.arg_str_array("ignore").unwrap_or_default();

        if patterns.is_empty() {
            log::warn!("[{}] '{}' missing args.patterns (expected array of strings); not starting", Self::id().bright_magenta(), self.sid);
            return;
        }

        let mut dns_on = self.cfg.arg_bool("dns").unwrap_or(false);
        if !dns_on {
            dns_on = patterns.iter().any(|p| Self::pattern_needs_dns(p));
        }
        let dns_ttl = self.cfg.arg_duration("dns-ttl").unwrap_or_else(|| Duration::from_secs(60));

        log::info!(
            "[{}] '{}' pulse {:?} dns={} ttl={:?} patterns={:?} ignore={:?}",
            Self::id().bright_magenta(),
            self.sid,
            pulse,
            dns_on,
            dns_ttl,
            patterns,
            ignores
        );

        let mut sensor = NetNotify::new(Some(NetNotifyConfig::default().pulse(pulse))).dns(dns_on).dns_ttl(dns_ttl);

        for p in &patterns {
            sensor.add(p);
        }
        for p in &ignores {
            sensor.ignore(p);
        }

        let mask = self.build_mask();
        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);

        let lstid = self.listener_id_with_tag();
        let sid = self.sid.clone();

        // bridge into omnitrace callback hub: return already-packaged SensorEvent JSON
        let mut hub = omnitrace_core::callbacks::CallbackHub::<NetNotifyEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: mask.bits(), sid, lstid, locked });
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
}

impl BridgeCb {
    fn make_eid(&self, action: &str, remote: &str) -> String {
        format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, remote, 0)
    }
}

#[async_trait::async_trait]
impl Callback<NetNotifyEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &NetNotifyEvent) -> Option<serde_json::Value> {
        let (action, conn) = match ev {
            NetNotifyEvent::Opened { conn } => ("opened", conn),
            NetNotifyEvent::Closed { conn } => ("closed", conn),
        };

        // EID: stable key => remote IP:port only
        let remote = conn.remote_dec.as_deref().unwrap_or("-");
        let eid = self.make_eid(action, remote);

        if self.locked && !libcommon::eidhub::get_eidhub().add("netnotify", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "netnotify",
            "data": {
                "action": action,
                "proto": conn.proto,
                "local_raw": conn.local,
                "remote_raw": conn.remote,
                "local": conn.local_dec,
                "remote": conn.remote_dec,
                "local_host": conn.local_host,
                "remote_host": conn.remote_host,
                "state": conn.state_dec,
            }
        }))
    }
}
