use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use nettools::{
    LiveHostnameBackend, NetTools, NetToolsConfig,
    events::{NetToolsEvent, NetToolsMask},
};
use omnitrace_core::{
    callbacks::Callback,
    sensor::{SensorCtx, SensorHandle},
};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Builds a `NetTools` hostname watcher for a `net.hostname` sensor instance.
///
/// The builder is stored behind an object so production code can use the live
/// backend while tests can inject a fake hostname backend without changing the
/// public sensor trait shape.
type NetHostnameFactory = Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync>;

/// Emits hostname change events from `omnitrace/nettools` into the `libsensors`
/// JSON event envelope.
///
/// This sensor is intentionally narrow:
/// - it watches only hostname transitions
/// - it does not enable any other `nettools` capability
/// - it keeps a stable event shape for higher layers
#[derive(Clone)]
pub struct NetHostnameSensor {
    sid: String,
    cfg: SensorConf,
    mk: NetHostnameFactory,
}

impl fmt::Debug for NetHostnameSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetHostnameSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetHostnameSensor {
    /// Builds a sensor instance with a custom `NetTools` factory.
    ///
    /// This exists so tests can inject a fake hostname backend while the public
    /// `Sensor::new` constructor keeps the normal production shape.
    #[cfg(test)]
    pub(crate) fn with_factory(id: String, cfg: SensorConf, mk: NetHostnameFactory) -> Self {
        Self { sid: id, cfg, mk }
    }

    /// Returns the listener id, including an optional `@tag` suffix.
    ///
    /// The returned value is used in generated event ids so tagged listeners
    /// stay distinct.
    pub(crate) fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_some() { "@" } else { "" }, self.cfg.tag().unwrap_or(""))
    }

    /// Builds a stable event id for a hostname change.
    ///
    /// The new hostname is used as the specific target portion so repeated
    /// transitions to the same hostname can be deduplicated when `locked` is
    /// enabled.
    #[cfg(test)]
    pub(crate) fn make_eid(&self, host: &str) -> String {
        format!("{}|{}|changed@{}|{}", self.sid, self.listener_id_with_tag(), host, 0)
    }

    /// Builds the live `NetTools` hostname watcher configuration.
    ///
    /// Only hostname watching is enabled here. All other `nettools` features
    /// stay disabled so this sensor remains focused and predictable.
    fn make_sensor(id: String, cfg: SensorConf) -> NetTools {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_secs(3)))
                .hostname(true)
                .routes(false)
                .default_routes(false)
                .nethealth(false)
                .sockets(false)
                .neighbours(false)
                .route_lookups(false)
                .throughput(false)
                .wifi(false),
        ));
        s.set_hostname_backend(LiveHostnameBackend);
        let _ = id;
        s
    }

    /// Starts the underlying `NetTools` watcher and forwards already packaged
    /// JSON envelopes through a result channel.
    ///
    /// This helper keeps the callback hub wiring in one place so production
    /// code and tests can reuse it consistently.
    async fn open(&self) -> (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) {
        let (tx, rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let mut hub = omnitrace_core::callbacks::CallbackHub::<NetToolsEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb::new(self.sid.clone(), self.listener_id_with_tag(), self.cfg.arg_bool("locked").unwrap_or(false)));
        SensorCtx::new(Arc::new(hub)).pipe(|(ctx, h)| (h, tokio::spawn((self.mk)(self.sid.clone(), self.cfg.clone()).run(ctx)), rx))
    }

    /// Runs the sensor until the first bridged event arrives or the timeout
    /// expires.
    ///
    /// This is used only by external unit tests so they can verify the bridge
    /// end to end without letting the live loop run forever.
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
    /// Passes a value through a single closure so nested construction can stay
    /// compact without temporary variables.
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[async_trait]
impl Sensor for NetHostnameSensor {
    /// Creates a production `net.hostname` sensor instance.
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { sid: id, cfg: cfg.clone(), mk: Arc::new(Self::make_sensor) }
    }

    /// Returns the public listener id for this sensor type.
    fn id() -> String {
        "net.hostname".to_string()
    }

    /// Runs the hostname watcher and emits bridged JSON events until the
    /// underlying watcher stops.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!(
            "[{}] '{}' watching hostnames with pulse {:?}",
            Self::id().bright_magenta(),
            self.sid,
            self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3))
        );

        let (_h, _t, mut rx): (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) = self.open().await;
        while let Some(v) = rx.recv().await {
            (emit)(v);
        }
    }
}

/// Bridges `NetTools` hostname events into the stable `libsensors` JSON
/// envelope.
struct BridgeCb {
    mask: u64,
    sid: String,
    lst: String,
    lock: bool,
}

impl BridgeCb {
    /// Creates a callback bridge for hostname events only.
    fn new(sid: String, lst: String, lock: bool) -> Self {
        Self { mask: NetToolsMask::HOSTNAME_CHANGED.bits(), sid, lst, lock }
    }

    /// Builds a stable event id for a hostname change.
    fn make_eid(&self, host: &str) -> String {
        format!("{}|{}|changed@{}|{}", self.sid, self.lst, host, 0)
    }
}

#[async_trait]
impl Callback<NetToolsEvent> for BridgeCb {
    /// Returns the `nettools` event mask accepted by this bridge.
    fn mask(&self) -> u64 {
        self.mask
    }

    /// Maps a `NetTools` hostname change event into the `libsensors` JSON
    /// envelope.
    async fn call(&self, ev: &NetToolsEvent) -> Option<serde_json::Value> {
        match ev {
            NetToolsEvent::HostnameChanged { old, new } => {
                if self.lock && !libcommon::eidhub::get_eidhub().add("net.hostname", &self.make_eid(new)).await {
                    return None;
                }

                Some(json!({
                    "eid": self.make_eid(new),
                    "sensor": self.sid,
                    "listener": "net.hostname",
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
