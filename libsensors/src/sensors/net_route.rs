use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use nettools::{
    LiveRouteBackend, NetTools, NetToolsConfig,
    events::{NetToolsEvent, NetToolsMask, RouteEntry},
};
use omnitrace_core::{
    callbacks::Callback,
    sensor::{SensorCtx, SensorHandle},
};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Builds a `NetTools` route watcher for a `net.route` sensor instance.
///
/// The builder stays injectable so tests can drive deterministic route changes
/// without changing the public `Sensor` constructor shape.
type NetRouteFactory = Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync>;

/// Emits route and default-route transitions from `omnitrace/nettools` into a
/// stable `libsensors` JSON envelope.
///
/// This sensor keeps route watching isolated from the rest of `nettools` so
/// models can subscribe only to route-related events.
#[derive(Clone)]
pub struct NetRouteSensor {
    sid: String,
    cfg: SensorConf,
    mk: NetRouteFactory,
}

impl fmt::Debug for NetRouteSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetRouteSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetRouteSensor {
    /// Builds a sensor instance with a custom `NetTools` factory for tests.
    #[cfg(test)]
    pub(crate) fn with_factory(id: String, cfg: SensorConf, mk: NetRouteFactory) -> Self {
        Self { sid: id, cfg, mk }
    }

    /// Returns the listener id, including an optional `@tag` suffix.
    pub(crate) fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_some() { "@" } else { "" }, self.cfg.tag().unwrap_or(""))
    }

    /// Builds the `nettools` event mask from route sensor options.
    ///
    /// If no options are given, all route and default-route transitions are
    /// watched.
    pub(crate) fn build_mask(&self) -> NetToolsMask {
        self.cfg.opts().iter().fold(
            if self.cfg.opts().is_empty() {
                NetToolsMask::ROUTE_ADDED
                    | NetToolsMask::ROUTE_REMOVED
                    | NetToolsMask::ROUTE_CHANGED
                    | NetToolsMask::DEFAULT_ROUTE_ADDED
                    | NetToolsMask::DEFAULT_ROUTE_REMOVED
                    | NetToolsMask::DEFAULT_ROUTE_CHANGED
            } else {
                NetToolsMask::empty()
            },
            |m, o| {
                m | match o.as_str() {
                    "route-added" => NetToolsMask::ROUTE_ADDED,
                    "route-removed" => NetToolsMask::ROUTE_REMOVED,
                    "route-changed" => NetToolsMask::ROUTE_CHANGED,
                    "default-added" => NetToolsMask::DEFAULT_ROUTE_ADDED,
                    "default-removed" => NetToolsMask::DEFAULT_ROUTE_REMOVED,
                    "default-changed" => NetToolsMask::DEFAULT_ROUTE_CHANGED,
                    _ => {
                        log::warn!("net.route '{}' unknown opt '{}'", self.sid, o);
                        NetToolsMask::empty()
                    }
                }
            },
        )
    }

    /// Builds a stable event id for a route transition.
    ///
    /// The destination stays the specific target portion so changed routes keep
    /// a stable identity.
    #[cfg(test)]
    pub(crate) fn make_eid(&self, act: &str, dst: &str) -> String {
        format!("{}|{}|{}@{}|{}", self.sid, self.listener_id_with_tag(), act, dst, 0)
    }

    /// Builds the live `NetTools` route watcher configuration.
    ///
    /// Only route and default-route watching are enabled here.
    fn make_sensor(id: String, cfg: SensorConf) -> NetTools {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_secs(3)))
                .hostname(false)
                .routes(true)
                .default_routes(true)
                .nethealth(false)
                .sockets(false)
                .neighbours(false)
                .route_lookups(false)
                .throughput(false)
                .wifi(false),
        ));
        s.set_route_backend(LiveRouteBackend);
        let _ = id;
        s
    }

    /// Starts the underlying route watcher and forwards bridged JSON envelopes
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
impl Sensor for NetRouteSensor {
    /// Creates a production `net.route` sensor instance.
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg: cfg.clone(), mk: Arc::new(Self::make_sensor) }
    }

    /// Returns the public listener id for this sensor type.
    fn id() -> String {
        "net.route".to_string()
    }

    /// Runs the route watcher and emits bridged JSON events until the
    /// underlying watcher stops.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!(
            "[{}] '{}' watching routes with pulse {:?} and opts {:?}",
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

/// Bridges route-related `NetTools` events into the stable `libsensors` JSON
/// envelope.
struct BridgeCb {
    mask: u64,
    sid: String,
    lst: String,
    lock: bool,
}

impl BridgeCb {
    /// Creates a callback bridge for route events.
    fn new(sid: String, lst: String, lock: bool, mask: u64) -> Self {
        Self { mask, sid, lst, lock }
    }

    /// Builds a stable event id for a route transition.
    fn make_eid(&self, act: &str, dst: &str) -> String {
        format!("{}|{}|{}@{}|{}", self.sid, self.lst, act, dst, 0)
    }

    /// Returns the destination to use as the event target.
    fn dst(route: &RouteEntry) -> &str {
        route.destination.as_str()
    }
}

#[async_trait]
impl Callback<NetToolsEvent> for BridgeCb {
    /// Returns the accepted `nettools` event mask for this bridge.
    fn mask(&self) -> u64 {
        self.mask
    }

    /// Maps route and default-route `NetTools` events into the `libsensors`
    /// JSON envelope.
    async fn call(&self, ev: &NetToolsEvent) -> Option<serde_json::Value> {
        match ev {
            NetToolsEvent::RouteAdded { route } => {
                self.emit("route-added", Self::dst(route), json!({ "action": "route-added", "route": route })).await
            }
            NetToolsEvent::RouteRemoved { route } => {
                self.emit("route-removed", Self::dst(route), json!({ "action": "route-removed", "route": route })).await
            }
            NetToolsEvent::RouteChanged { old, new } => {
                self.emit("route-changed", Self::dst(new), json!({ "action": "route-changed", "old": old, "new": new })).await
            }
            NetToolsEvent::DefaultRouteAdded { route } => {
                self.emit("default-added", Self::dst(route), json!({ "action": "default-added", "route": route })).await
            }
            NetToolsEvent::DefaultRouteRemoved { route } => {
                self.emit("default-removed", Self::dst(route), json!({ "action": "default-removed", "route": route })).await
            }
            NetToolsEvent::DefaultRouteChanged { old, new } => {
                self.emit("default-changed", Self::dst(new), json!({ "action": "default-changed", "old": old, "new": new })).await
            }
            _ => None,
        }
    }
}

impl BridgeCb {
    /// Emits one fully packaged route event, honouring optional lock-based
    /// duplicate suppression.
    async fn emit(&self, act: &str, dst: &str, data: serde_json::Value) -> Option<serde_json::Value> {
        if self.lock && !libcommon::eidhub::get_eidhub().add("net.route", &self.make_eid(act, dst)).await {
            return None;
        }

        Some(json!({
            "eid": self.make_eid(act, dst),
            "sensor": self.sid,
            "listener": "net.route",
            "data": data,
        }))
    }
}
