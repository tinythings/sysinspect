use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use omnitrace_core::callbacks::Callback;
use serde_json::json;
use socktray::events::{SockTrayEvent, SockTrayMask};
use socktray::{SockTray, SockTrayConfig};
use std::{fmt, time::Duration};
use tokio::sync::mpsc;

pub struct SockTraySensor {
    sid: String,
    cfg: SensorConf,
}

impl fmt::Debug for SockTraySensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SockTraySensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl SockTraySensor {
    pub(crate) fn build_mask(&self) -> SockTrayMask {
        let mut mask = SockTrayMask::empty();
        if self.cfg.opts().is_empty() {
            mask |= SockTrayMask::OPENED | SockTrayMask::CLOSED;
        } else {
            for o in self.cfg.opts() {
                match o.as_str() {
                    "opened" => mask |= SockTrayMask::OPENED,
                    "closed" => mask |= SockTrayMask::CLOSED,
                    _ => log::warn!("socknotify '{}' unknown opt '{}'", self.sid, o),
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
impl Sensor for SockTraySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String {
        "socknotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(1));
        let dns = self.cfg.arg_bool("dns").unwrap_or(false);
        let dns_ttl = self.cfg.arg_duration("dns-ttl").unwrap_or_else(|| Duration::from_secs(60));
        let skip_reverse_dns = self.cfg.arg_bool("skip-reverse-dns").or_else(|| self.cfg.arg_bool("skip_reverse_dns")).unwrap_or(false);
        let locked = self.cfg.arg_bool("locked").unwrap_or(false);

        let patterns = self.cfg.arg_str_array("patterns").unwrap_or_default();
        let ignores = self.cfg.arg_str_array("ignore").unwrap_or_default();

        log::info!(
            "[{}] '{}' pulse {:?} dns={} ttl={:?} skip-reverse-dns={} patterns={:?} ignore={:?}",
            Self::id().bright_magenta(),
            self.sid,
            pulse,
            dns,
            dns_ttl,
            skip_reverse_dns,
            patterns,
            ignores
        );

        let mut sensor = SockTray::new(Some(SockTrayConfig::default().pulse(pulse).dns(dns).dns_ttl(dns_ttl).skip_reverse_dns(skip_reverse_dns)));
        for p in &patterns {
            sensor.add(p);
        }
        for p in &ignores {
            sensor.ignore(p);
        }

        let mask = self.build_mask();
        let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let lstid = self.listener_id_with_tag();

        let mut hub = omnitrace_core::callbacks::CallbackHub::<SockTrayEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb { mask: mask.bits(), sid: self.sid.clone(), lstid, locked });
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

#[async_trait::async_trait]
impl Callback<SockTrayEvent> for BridgeCb {
    fn mask(&self) -> u64 {
        self.mask
    }

    async fn call(&self, ev: &SockTrayEvent) -> Option<serde_json::Value> {
        let (action, sock) = match ev {
            SockTrayEvent::Opened { sock } => ("opened", sock),
            SockTrayEvent::Closed { sock } => ("closed", sock),
        };

        let remote = sock.remote_dec.as_deref().unwrap_or(&sock.remote);
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.lstid, action, remote, 0);

        if self.locked && !libcommon::eidhub::get_eidhub().add("socknotify", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "socknotify",
            "data": {
                "action": action,
                "proto": sock.proto,
                "local_raw": sock.local,
                "remote_raw": sock.remote,
                "local": sock.local_dec,
                "remote": sock.remote_dec,
                "remote_host": sock.remote_host,
                "state": sock.state_dec,
            },
        }))
    }
}
