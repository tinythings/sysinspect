use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use nettools::{
    LiveWifiBackend, NetTools, NetToolsConfig,
    events::{NetToolsEvent, NetToolsMask, WifiDetails},
};
use omnitrace_core::{
    callbacks::Callback,
    sensor::{SensorCtx, SensorHandle},
};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Builds a `NetTools` Wi-Fi watcher for a `net.wifi` sensor instance.
///
/// The builder stays injectable so tests can drive deterministic Wi-Fi changes
/// without changing the public `Sensor` constructor shape.
type NetWifiFactory = Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync>;

/// Emits Wi-Fi transitions from `omnitrace/nettools` into a stable
/// `libsensors` JSON envelope.
///
/// This sensor watches Wi-Fi state only and keeps the event contract small and
/// focused.
#[derive(Clone)]
pub struct NetWifiSensor {
    sid: String,
    cfg: SensorConf,
    mk: NetWifiFactory,
}

impl fmt::Debug for NetWifiSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetWifiSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetWifiSensor {
    /// Builds a sensor instance with a custom `NetTools` factory for tests.
    #[cfg(test)]
    pub(crate) fn with_factory(id: String, cfg: SensorConf, mk: NetWifiFactory) -> Self {
        Self { sid: id, cfg, mk }
    }

    /// Returns the listener id, including an optional `@tag` suffix.
    pub(crate) fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_some() { "@" } else { "" }, self.cfg.tag().unwrap_or(""))
    }

    /// Builds the `nettools` event mask from Wi-Fi sensor options.
    ///
    /// If no options are given, all Wi-Fi transitions are watched.
    pub(crate) fn build_mask(&self) -> NetToolsMask {
        self.cfg.opts().iter().fold(
            if self.cfg.opts().is_empty() {
                NetToolsMask::WIFI_ADDED | NetToolsMask::WIFI_REMOVED | NetToolsMask::WIFI_CHANGED
            } else {
                NetToolsMask::empty()
            },
            |m, o| {
                m | match o.as_str() {
                    "connected" => NetToolsMask::WIFI_ADDED,
                    "disconnected" => NetToolsMask::WIFI_REMOVED,
                    "changed" => NetToolsMask::WIFI_CHANGED,
                    _ => {
                        log::warn!("net.wifi '{}' unknown opt '{}'", self.sid, o);
                        NetToolsMask::empty()
                    }
                }
            },
        )
    }

    /// Builds a stable event id for a Wi-Fi transition.
    ///
    /// The interface stays the specific target portion so Wi-Fi transitions keep
    /// a stable identity per interface.
    #[cfg(test)]
    pub(crate) fn make_eid(&self, act: &str, ifc: &str) -> String {
        format!("{}|{}|{}@{}|{}", self.sid, self.listener_id_with_tag(), act, ifc, 0)
    }

    /// Builds the live `NetTools` Wi-Fi watcher configuration.
    ///
    /// Only Wi-Fi watching is enabled here.
    fn make_sensor(id: String, cfg: SensorConf) -> NetTools {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_secs(3)))
                .hostname(false)
                .routes(false)
                .default_routes(false)
                .nethealth(false)
                .sockets(false)
                .neighbours(false)
                .route_lookups(false)
                .throughput(false)
                .wifi(true),
        ));
        s.set_wifi_backend(LiveWifiBackend);
        let _ = id;
        s
    }

    /// Starts the underlying Wi-Fi watcher and forwards bridged JSON envelopes
    /// through a result channel.
    async fn open(&self) -> (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) {
        let (tx, rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let mut hub = omnitrace_core::callbacks::CallbackHub::<NetToolsEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb::new(self.sid.clone(), self.listener_id_with_tag(), self.cfg.arg_bool("locked").unwrap_or(false), self.build_mask().bits()));
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
impl Sensor for NetWifiSensor {
    /// Creates a production `net.wifi` sensor instance.
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg: cfg.clone(), mk: Arc::new(Self::make_sensor) }
    }

    /// Returns the public listener id for this sensor type.
    fn id() -> String {
        "net.wifi".to_string()
    }

    /// Runs the Wi-Fi watcher and emits bridged JSON events until the
    /// underlying watcher stops.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!(
            "[{}] '{}' watching wifi with pulse {:?} and opts {:?}",
            Self::id().bright_magenta(),
            self.sid,
            self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3)),
            self.cfg.opts()
        );

        let (_h, _t, mut rx): (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) = self.open().await;
        while let Some(v) = rx.recv().await {
            (emit)(v);
        }
    }
}

/// Bridges Wi-Fi `NetTools` events into the stable `libsensors` JSON envelope.
struct BridgeCb {
    mask: u64,
    sid: String,
    lst: String,
    lock: bool,
}

impl BridgeCb {
    /// Creates a callback bridge for Wi-Fi events.
    fn new(sid: String, lst: String, lock: bool, mask: u64) -> Self {
        Self { mask, sid, lst, lock }
    }

    /// Builds a stable event id for a Wi-Fi transition.
    fn make_eid(&self, act: &str, ifc: &str) -> String {
        format!("{}|{}|{}@{}|{}", self.sid, self.lst, act, ifc, 0)
    }

    /// Returns the interface name to use as the event target.
    fn ifc(w: &WifiDetails) -> &str {
        w.iface.as_str()
    }

    /// Emits one fully packaged Wi-Fi event, honouring optional lock-based
    /// duplicate suppression.
    async fn emit(&self, act: &str, ifc: &str, data: serde_json::Value) -> Option<serde_json::Value> {
        if self.lock && !libcommon::eidhub::get_eidhub().add("net.wifi", &self.make_eid(act, ifc)).await {
            return None;
        }

        Some(json!({
            "eid": self.make_eid(act, ifc),
            "sensor": self.sid,
            "listener": "net.wifi",
            "data": data,
        }))
    }
}

#[async_trait]
impl Callback<NetToolsEvent> for BridgeCb {
    /// Returns the accepted `nettools` event mask for this bridge.
    fn mask(&self) -> u64 {
        self.mask
    }

    /// Maps Wi-Fi `NetTools` events into the `libsensors` JSON envelope.
    async fn call(&self, ev: &NetToolsEvent) -> Option<serde_json::Value> {
        match ev {
            NetToolsEvent::WifiAdded { wifi } => self.emit("connected", Self::ifc(wifi), json!({ "action": "connected", "wifi": wifi })).await,
            NetToolsEvent::WifiRemoved { wifi } => {
                self.emit("disconnected", Self::ifc(wifi), json!({ "action": "disconnected", "wifi": wifi })).await
            }
            NetToolsEvent::WifiChanged { old, new } => {
                self.emit("changed", Self::ifc(new), json!({ "action": "changed", "old": old, "new": new })).await
            }
            _ => None,
        }
    }
}
