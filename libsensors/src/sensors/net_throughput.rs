use crate::{
    argparse::SensorArgs,
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use nettools::{
    LiveThroughputBackend, NetTools, NetToolsConfig,
    events::{NetToolsEvent, NetToolsMask, ThroughputSample},
};
use omnitrace_core::{
    callbacks::Callback,
    sensor::{SensorCtx, SensorHandle},
};
use serde_json::json;
use std::{fmt, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Builds a `NetTools` throughput watcher for a `net.throughput` sensor
/// instance.
///
/// The builder stays injectable so tests can drive deterministic counter
/// changes without changing the public `Sensor` constructor shape.
type NetThroughputFactory = Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync>;

/// Emits throughput updates from `omnitrace/nettools` into a stable
/// `libsensors` JSON envelope.
///
/// This sensor watches interface counters and calculated rates only.
#[derive(Clone)]
pub struct NetThroughputSensor {
    sid: String,
    cfg: SensorConf,
    mk: NetThroughputFactory,
}

impl fmt::Debug for NetThroughputSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetThroughputSensor").field("sid", &self.sid).field("listener", &self.cfg.listener()).finish()
    }
}

impl NetThroughputSensor {
    /// Builds a sensor instance with a custom `NetTools` factory for tests.
    #[cfg(test)]
    pub(crate) fn with_factory(id: String, cfg: SensorConf, mk: NetThroughputFactory) -> Self {
        Self { sid: id, cfg, mk }
    }

    /// Returns the listener id, including an optional `@tag` suffix.
    pub(crate) fn listener_id_with_tag(&self) -> String {
        format!("{}{}{}", Self::id(), if self.cfg.tag().is_some() { "@" } else { "" }, self.cfg.tag().unwrap_or(""))
    }

    /// Builds the `nettools` event mask from throughput sensor options.
    ///
    /// Throughput currently emits one event class only, so the mask is always
    /// `THROUGHPUT_UPDATED`.
    pub(crate) fn build_mask(&self) -> NetToolsMask {
        NetToolsMask::THROUGHPUT_UPDATED
    }

    /// Builds a stable event id for a throughput update.
    ///
    /// The interface stays the specific target portion so throughput updates
    /// keep a stable identity per interface.
    #[cfg(test)]
    pub(crate) fn make_eid(&self, ifc: &str) -> String {
        format!("{}|{}|updated@{}|{}", self.sid, self.listener_id_with_tag(), ifc, 0)
    }

    /// Builds the live `NetTools` throughput watcher configuration.
    ///
    /// Only throughput watching is enabled here.
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
                .throughput(true)
                .wifi(false),
        ));
        s.set_throughput_backend(LiveThroughputBackend);
        let _ = id;
        s
    }

    /// Starts the underlying throughput watcher and forwards bridged JSON
    /// envelopes through a result channel.
    async fn open(&self) -> (SensorHandle, tokio::task::JoinHandle<()>, mpsc::Receiver<serde_json::Value>) {
        let (tx, rx) = mpsc::channel::<serde_json::Value>(0xfff);
        let mut hub = omnitrace_core::callbacks::CallbackHub::<NetToolsEvent>::new();
        hub.set_result_channel(tx);
        hub.add(BridgeCb::new(
            self.sid.clone(),
            self.listener_id_with_tag(),
            self.cfg.arg_bool("locked").unwrap_or(false),
            self.build_mask().bits(),
        ));
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
impl Sensor for NetThroughputSensor {
    /// Creates a production `net.throughput` sensor instance.
    fn new(id: String, cfg: SensorConf) -> Self {
        Self {
            sid: id,
            cfg: cfg.clone(),
            mk: Arc::new(Self::make_sensor),
        }
    }

    /// Returns the public listener id for this sensor type.
    fn id() -> String {
        "net.throughput".to_string()
    }

    /// Runs the throughput watcher and emits bridged JSON events until the
    /// underlying watcher stops.
    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!(
            "[{}] '{}' watching throughput with pulse {:?}",
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

/// Bridges throughput `NetTools` events into the stable `libsensors` JSON
/// envelope.
struct BridgeCb {
    mask: u64,
    sid: String,
    lst: String,
    lock: bool,
}

impl BridgeCb {
    /// Creates a callback bridge for throughput events.
    fn new(sid: String, lst: String, lock: bool, mask: u64) -> Self {
        Self { mask, sid, lst, lock }
    }

    /// Builds a stable event id for a throughput update.
    fn make_eid(&self, ifc: &str) -> String {
        format!("{}|{}|updated@{}|{}", self.sid, self.lst, ifc, 0)
    }

    /// Emits one fully packaged throughput event, honouring optional lock-based
    /// duplicate suppression.
    async fn emit(&self, sample: &ThroughputSample) -> Option<serde_json::Value> {
        if self.lock && !libcommon::eidhub::get_eidhub().add("net.throughput", &self.make_eid(sample.iface.as_str())).await {
            return None;
        }

        Some(json!({
            "eid": self.make_eid(sample.iface.as_str()),
            "sensor": self.sid,
            "listener": "net.throughput",
            "data": {
                "action": "updated",
                "sample": sample,
            },
        }))
    }
}

#[async_trait]
impl Callback<NetToolsEvent> for BridgeCb {
    /// Returns the accepted `nettools` event mask for this bridge.
    fn mask(&self) -> u64 {
        self.mask
    }

    /// Maps throughput `NetTools` events into the `libsensors` JSON envelope.
    async fn call(&self, ev: &NetToolsEvent) -> Option<serde_json::Value> {
        match ev {
            NetToolsEvent::ThroughputUpdated { sample } => self.emit(sample).await,
            _ => None,
        }
    }
}
