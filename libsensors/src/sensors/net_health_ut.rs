use crate::{
    argparse::SensorArgs,
    sensors::{net_health::NetHealthSensor, sensor::Sensor},
    sspec::SensorConf,
};
use async_trait::async_trait;
use nettools::{NetHealthBackend, NetTools, NetToolsConfig, events::NetHealthTarget};
use serde_json::{from_value, json};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone)]
enum Probe {
    Ok(u64),
    Err,
}

/// Returns a `net.health` sensor configuration for tests.
fn mk_cfg(tag: Option<&str>, lock: bool, ms: u64) -> SensorConf {
    from_value(json!({
        "listener": "net.health",
        "tag": tag,
        "interval": {"secs": 0, "nanos": ms * 1_000_000},
        "args": {
            "locked": lock,
            "targets": ["1.1.1.1:53", "8.8.8.8:53"],
            "window": 2,
            "timeout": "100ms",
            "latency-degraded-ms": 40,
            "loss-degraded-pct": 25
        }
    }))
    .unwrap()
}

/// Provides a deterministic probe sequence for `NetTools` tests.
struct SeqProbe {
    q: Mutex<VecDeque<Probe>>,
}

impl SeqProbe {
    /// Creates a probe backend from a queued sequence of results.
    fn new(v: Vec<Probe>) -> Self {
        Self { q: Mutex::new(v.into()) }
    }
}

#[async_trait]
impl NetHealthBackend for SeqProbe {
    /// Returns the next queued probe result or a timeout-like error if empty.
    async fn probe(&self, _: &NetHealthTarget, _: Duration) -> io::Result<Duration> {
        match self.q.lock().unwrap().pop_front().unwrap_or(Probe::Err) {
            Probe::Ok(ms) => Ok(Duration::from_millis(ms)),
            Probe::Err => Err(io::Error::new(io::ErrorKind::TimedOut, "no probe")),
        }
    }
}

/// Creates a `NetTools` net-health watcher with a fake probe backend.
fn mk_factory(v: Vec<Probe>) -> Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync> {
    Arc::new(move |_, cfg| {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_millis(10)))
                .hostname(false)
                .routes(false)
                .default_routes(false)
                .nethealth(true)
                .nethealth_window(cfg.arg_u64("window").unwrap_or(2) as usize)
                .nethealth_timeout(cfg.arg_duration("timeout").unwrap_or_else(|| Duration::from_millis(100)))
                .nethealth_latency_degraded_ms(cfg.arg_u64("latency-degraded-ms").unwrap_or(40))
                .nethealth_loss_degraded_pct(cfg.arg_u64("loss-degraded-pct").unwrap_or(25) as u8)
                .sockets(false)
                .neighbours(false)
                .route_lookups(false)
                .throughput(false)
                .wifi(false),
        ));
        s.set_nethealth_backend(SeqProbe::new(v.clone()));
        s.add_nethealth_target("1.1.1.1", 53);
        s.add_nethealth_target("8.8.8.8", 53);
        s
    })
}

#[test]
fn listener_id_without_tag() {
    assert_eq!(NetHealthSensor::new("sid".to_string(), mk_cfg(None, false, 10)).listener_id_with_tag(), "net.health");
}

#[test]
fn listener_id_with_tag() {
    assert_eq!(NetHealthSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).listener_id_with_tag(), "net.health@car");
}

#[test]
fn targets_parse_from_args() {
    let t = NetHealthSensor::new("sid".to_string(), mk_cfg(None, false, 10)).targets();
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].host, "1.1.1.1");
    assert_eq!(t[0].port, 53);
}

#[test]
fn make_eid_with_tag() {
    assert_eq!(NetHealthSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).make_eid("degraded"), "sid|net.health@car|changed@degraded|0");
}

#[tokio::test]
async fn run_returns_early_when_targets_missing_and_does_not_emit() {
    let s = NetHealthSensor::new(
        "sid".to_string(),
        from_value(json!({
            "listener": "net.health",
            "interval": {"secs": 0, "nanos": 10_000_000},
            "args": {}
        }))
        .unwrap(),
    );
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c = n.clone();
    s.run(&move |_| {
        c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    })
    .await;
    assert_eq!(n.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn recv_once_emits_nethealth_changed_envelope() {
    let s = NetHealthSensor::with_factory(
        "sid".to_string(),
        mk_cfg(Some("car"), false, 10),
        mk_factory(vec![Probe::Ok(10), Probe::Ok(10), Probe::Ok(80), Probe::Err]),
    );
    let v = s.recv_once(Duration::from_millis(200)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.health@car|changed@degraded|0");
    assert_eq!(v["listener"], "net.health");
    assert_eq!(v["data"]["action"], "changed");
    assert_eq!(v["data"]["old"]["level"], "Healthy");
    assert_eq!(v["data"]["new"]["level"], "Degraded");
}
