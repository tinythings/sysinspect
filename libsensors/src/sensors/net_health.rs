use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use nettools::{
    LiveNetHealthBackend, NetTools, NetToolsConfig,
    events::{NetHealthTarget, NetToolsEvent, NetToolsMask},
};
use omnitrace_core::{
    callbacks::Callback,
    sensor::{SensorCtx, SensorHandle},
};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Builds a `NetTools` net-health watcher for a `net.health` sensor instance.
///
/// The builder stays injectable so tests can drive deterministic health
/// transitions without changing the public `Sensor` constructor shape.
type NetHealthFactory = Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync>;

/// Emits network health transitions from `omnitrace/nettools` into a stable
/// `libsensors` JSON envelope.
#[derive(Clone)]
pub struct NetHealthSensor {
    sid: String,
    cfg: SensorConf,
    mk: NetHealthFactory,
}

impl fmt::Debug for NetHealthSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetHealthSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetHealthSensor {
    /// Builds a sensor instance with a custom `NetTools` factory for tests.
    #[cfg(test)]
    pub(crate) fn with_factory(id: String, cfg: SensorConf, mk: NetHealthFactory) -> Self {
        Self { sid: id, cfg, mk }
    }

    /// Returns the listener id, including an optional `@tag` suffix.
    pub(crate) fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_some() { "@" } else { "" }, self.cfg.tag().unwrap_or(""))
    }

    /// Builds a stable event id for a net-health transition.
    #[cfg(test)]
    pub(crate) fn make_eid(&self, lvl: &str) -> String {
        format!("{}|{}|changed@{}|{}", self.sid, self.listener_id_with_tag(), lvl, 0)
    }

    /// Parses probe targets from `args.targets`.
    pub(crate) fn targets(&self) -> Vec<NetHealthTarget> {
        self.cfg
            .arg_str_array("targets")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| {
                v.rsplit_once(':').and_then(|(h, p)| {
                    p.parse::<u16>().ok().filter(|_| !h.trim().is_empty()).map(|p| NetHealthTarget { host: h.trim().to_string(), port: p })
                })
            })
            .collect()
    }

    /// Builds the live `NetTools` net-health watcher configuration.
    fn make_sensor(id: String, cfg: SensorConf) -> NetTools {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_secs(3)))
                .hostname(false)
                .routes(false)
                .default_routes(false)
                .nethealth(true)
                .nethealth_window(cfg.arg_u64("window").unwrap_or(4) as usize)
                .nethealth_timeout(cfg.arg_duration("timeout").unwrap_or_else(|| Duration::from_secs(2)))
                .nethealth_latency_degraded_ms(cfg.arg_u64("latency-degraded-ms").unwrap_or(400))
                .nethealth_loss_degraded_pct(cfg.arg_u64("loss-degraded-pct").unwrap_or(25) as u8)
                .sockets(false)
                .neighbours(false)
                .route_lookups(false)
                .throughput(false)
                .wifi(false),
        ));
        s.set_nethealth_backend(LiveNetHealthBackend);
        Self::with_targets(&cfg, &mut s);
        let _ = id;
        s
    }

    /// Adds configured targets into a `NetTools` instance.
    fn with_targets(cfg: &SensorConf, s: &mut NetTools) {
        cfg.arg_str_array("targets")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| {
                v.rsplit_once(':').and_then(|(h, p)| p.parse::<u16>().ok().filter(|_| !h.trim().is_empty()).map(|p| (h.trim().to_string(), p)))
            })
            .for_each(|(h, p)| s.add_nethealth_target(h, p));
    }

    /// Starts the underlying net-health watcher and forwards bridged JSON
    /// envelopes through a result channel.
    async fn open(&self) -> (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) {
        let (tx, rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let mut hub = omnitrace_core::callbacks::CallbackHub::<NetToolsEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb::new(self.sid.clone(), self.listener_id_with_tag(), self.cfg.arg_bool("locked").unwrap_or(false)));
        SensorCtx::new(Arc::new(hub)).pipe(|(ctx, h)| (h, tokio::spawn((self.mk)(self.sid.clone(), self.cfg.clone()).run(ctx)), rx))
    }

    /// Waits for one bridged event and then shuts the watcher down.
    #[cfg(test)]
    pub(crate) async fn recv_once(&self, wait: Duration) -> Option<serde_json::Value> {
        let (h, t, mut rx): (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) = self.open().await;
        let r = tokio::time::timeout(wait, rx.recv()).await.ok().flatten();
        h.shutdown();
        let _ = t.await;
        r
    }
}

trait Pipe: Sized {
    /// Passes a value through one closure so nested construction can stay
    /// compact without temporary variables.
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[async_trait]
impl Sensor for NetHealthSensor {
    /// Creates a production `net.health` sensor instance.
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg: cfg.clone(), mk: Arc::new(Self::make_sensor) }
    }

    /// Returns the public listener id for this sensor type.
    fn id() -> String {
        "net.health".to_string()
    }

    /// Runs the net-health watcher and emits bridged JSON events until the
    /// underlying watcher stops.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        if self.targets().is_empty() {
            log::warn!("[{}] '{}' missing args.targets; not starting", Self::id().bright_magenta(), self.sid);
            return;
        }

        log::info!(
            "[{}] '{}' watching net health with pulse {:?} and targets {:?}",
            Self::id().bright_magenta(),
            self.sid,
            self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3)),
            self.cfg.arg_str_array("targets").unwrap_or_default()
        );

        let (_h, _t, mut rx): (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) = self.open().await;
        while let Some(v) = rx.recv().await {
            (emit)(v);
        }
    }
}

/// Bridges net-health `NetTools` events into the stable `libsensors` JSON
/// envelope.
struct BridgeCb {
    mask: u64,
    sid: String,
    lst: String,
    lock: bool,
}

impl BridgeCb {
    /// Creates a callback bridge for net-health events.
    fn new(sid: String, lst: String, lock: bool) -> Self {
        Self { mask: NetToolsMask::NETHEALTH_CHANGED.bits(), sid, lst, lock }
    }

    /// Builds a stable event id for a net-health transition.
    fn make_eid(&self, lvl: &str) -> String {
        format!("{}|{}|changed@{}|{}", self.sid, self.lst, lvl, 0)
    }
}

#[async_trait]
impl Callback<NetToolsEvent> for BridgeCb {
    /// Returns the accepted `nettools` event mask for this bridge.
    fn mask(&self) -> u64 {
        self.mask
    }

    /// Maps net-health `NetTools` events into the `libsensors` JSON envelope.
    async fn call(&self, ev: &NetToolsEvent) -> Option<serde_json::Value> {
        match ev {
            NetToolsEvent::NetHealthChanged { old, new } => {
                let lvl = format!("{:?}", new.level).to_lowercase();
                if self.lock && !libcommon::eidhub::get_eidhub().add("net.health", &self.make_eid(lvl.as_str())).await {
                    return None;
                }

                Some(json!({
                    "eid": self.make_eid(lvl.as_str()),
                    "sensor": self.sid,
                    "listener": "net.health",
                    "data": {
                        "action": "changed",
                        "old": old,
                        "new": new,
                    }
                }))
            }
            _ => None,
        }
    }
}
