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
use omnitrace_core::callbacks::Callback;
#[cfg(not(target_os = "freebsd"))]
use socktray::events::SockTrayEvent;
#[cfg(not(target_os = "freebsd"))]
pub(crate) use socktray::events::SockTrayMask;
#[cfg(not(target_os = "freebsd"))]
use socktray::{SockTray, SockTrayConfig};
#[cfg(not(target_os = "freebsd"))]
use tokio::sync::mpsc;

#[cfg(target_os = "freebsd")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SockTrayMask(u8);

#[cfg(target_os = "freebsd")]
impl SockTrayMask {
    pub(crate) const OPENED: Self = Self(0b01);
    pub(crate) const CLOSED: Self = Self(0b10);

    pub(crate) fn empty() -> Self {
        Self(0)
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(target_os = "freebsd")]
impl std::ops::BitOrAssign for SockTrayMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(target_os = "freebsd")]
#[derive(Clone, Debug)]
struct FreeBsdSockRow {
    proto: String,
    local: String,
    remote: String,
    state: String,
}

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
            mask |= SockTrayMask::OPENED;
            mask |= SockTrayMask::CLOSED;
        } else {
            for opt in self.cfg.opts() {
                match opt.as_str() {
                    "opened" => mask |= SockTrayMask::OPENED,
                    "closed" => mask |= SockTrayMask::CLOSED,
                    _ => log::warn!("net.socket '{}' unknown opt '{}'", self.sid, opt),
                }
            }
        }
        mask
    }

    fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_none() { "" } else { "@" }, self.cfg.tag().unwrap_or(""))
    }

    #[cfg(target_os = "freebsd")]
    fn freebsd_rows(&self) -> Vec<FreeBsdSockRow> {
        Command::new("sockstat")
            .args(["-46"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .map(|stdout| {
                stdout
                    .lines()
                    .skip(1)
                    .filter_map(|line| {
                        let fields = line.split_whitespace().collect::<Vec<_>>();
                        (fields.len() >= 6).then(|| FreeBsdSockRow {
                            proto: fields[4].to_string(),
                            local: fields[5].to_string(),
                            remote: fields.get(6).unwrap_or(&"*:*").to_string(),
                            state: if fields.len() >= 8 { fields[7..].join(" ") } else { "".to_string() },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "freebsd")]
    fn allowed(&self, row: &FreeBsdSockRow) -> bool {
        let matches_patterns = self.cfg.arg_str_array("patterns").unwrap_or_default();
        let matches_ignores = self.cfg.arg_str_array("ignore").unwrap_or_default();
        let text = format!("{} {} {} {}", row.proto, row.local, row.remote, row.state);
        (matches_patterns.is_empty() || matches_patterns.iter().any(|pattern| text.contains(pattern)))
            && !matches_ignores.iter().any(|pattern| text.contains(pattern))
    }

    #[cfg(target_os = "freebsd")]
    async fn emit_freebsd_event(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync), action: &str, row: &FreeBsdSockRow, locked: bool) {
        let eid = format!("{}|{}|{}@{}|{}", self.sid, self.listener_id_with_tag(), action, row.remote, 0);
        if locked && !libcommon::eidhub::get_eidhub().add("net.socket", &eid).await {
            return;
        }

        (emit)(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "net.socket",
            "data": {
                "action": action,
                "proto": row.proto,
                "local_raw": row.local,
                "remote_raw": row.remote,
                "local": row.local,
                "remote": row.remote,
                "remote_host": serde_json::Value::Null,
                "state": row.state,
            },
        }));
    }
}

#[async_trait]
impl Sensor for SockTraySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "net.socket".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        #[cfg(not(target_os = "freebsd"))]
        {
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
            for pattern in &patterns {
                sensor.add(pattern);
            }
            for pattern in &ignores {
                sensor.ignore(pattern);
            }

            let mask = self.build_mask();
            let (tx, mut rx) = mpsc::channel::<serde_json::Value>(0xfff);
            let mut hub = omnitrace_core::callbacks::CallbackHub::<SockTrayEvent>::new();
            hub.set_result_channel(tx);
            hub.add(BridgeCb {
                mask: mask.bits(),
                sid: self.sid.clone(),
                lstid: self.listener_id_with_tag(),
                locked,
            });
            let (ctx, _handle) = omnitrace_core::sensor::SensorCtx::new(std::sync::Arc::new(hub));
            tokio::spawn(sensor.run(ctx));

            while let Some(value) = rx.recv().await {
                (emit)(value);
            }
        }

        #[cfg(target_os = "freebsd")]
        {
            let pulse = self.cfg.interval().unwrap_or_else(|| Duration::from_secs(1));
            let locked = self.cfg.arg_bool("locked").unwrap_or(false);
            let mask = self.build_mask();
            let mut seen = HashMap::<String, FreeBsdSockRow>::new();
            let mut tick = time::interval(pulse);

            log::info!("[{}] '{}' pulse {:?} via sockstat", Self::id().bright_magenta(), self.sid, pulse);

            loop {
                tick.tick().await;
                let current = self
                    .freebsd_rows()
                    .into_iter()
                    .filter(|row| self.allowed(row))
                    .map(|row| (format!("{}|{}|{}", row.proto, row.local, row.remote), row))
                    .collect::<HashMap<_, _>>();

                if mask.contains(SockTrayMask::OPENED) {
                    for row in current.iter().filter(|(key, _)| !seen.contains_key(*key)).map(|(_, row)| row) {
                        self.emit_freebsd_event(emit, "opened", row, locked).await;
                    }
                }

                if mask.contains(SockTrayMask::CLOSED) {
                    for row in seen.iter().filter(|(key, _)| !current.contains_key(*key)).map(|(_, row)| row) {
                        self.emit_freebsd_event(emit, "closed", row, locked).await;
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

        if self.locked && !libcommon::eidhub::get_eidhub().add("net.socket", &eid).await {
            return None;
        }

        Some(json!({
            "eid": eid,
            "sensor": self.sid,
            "listener": "net.socket",
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
