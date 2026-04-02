use crate::{
    sensors::{net_throughput::NetThroughputSensor, sensor::Sensor},
    sspec::SensorConf,
};
use nettools::{NetTools, NetToolsConfig, ThroughputBackend, events::InterfaceCounters};
use serde_json::{from_value, json};
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Returns a `net.throughput` sensor configuration for tests.
fn mk_cfg(tag: Option<&str>, lock: bool, ms: u64) -> SensorConf {
    from_value(json!({
        "listener": "net.throughput",
        "tag": tag,
        "interval": {"secs": 0, "nanos": ms * 1_000_000},
        "args": {
            "locked": lock
        }
    }))
    .unwrap()
}

/// Creates a simple interface counter snapshot for tests.
fn mk_cnt(ifc: &str, rx_b: u64, rx_p: u64, tx_b: u64, tx_p: u64) -> InterfaceCounters {
    InterfaceCounters {
        iface: ifc.to_string(),
        rx_bytes: rx_b,
        rx_packets: rx_p,
        rx_errors: 0,
        rx_drops: 0,
        tx_bytes: tx_b,
        tx_packets: tx_p,
        tx_errors: 0,
        tx_drops: 0,
    }
}

/// Provides a deterministic throughput snapshot sequence for `NetTools` tests.
struct SeqTp {
    q: Mutex<VecDeque<HashMap<String, InterfaceCounters>>>,
    d: HashMap<String, InterfaceCounters>,
}

impl SeqTp {
    /// Creates a backend that returns queued counter snapshots and then keeps
    /// the last one stable.
    fn new(v: Vec<HashMap<String, InterfaceCounters>>) -> Self {
        Self { d: v.last().cloned().unwrap_or_default(), q: Mutex::new(v.into()) }
    }
}

impl ThroughputBackend for SeqTp {
    /// Returns the next queued counter snapshot or the last stable snapshot.
    fn list(&self) -> io::Result<HashMap<String, InterfaceCounters>> {
        Ok(self.q.lock().unwrap().pop_front().unwrap_or_else(|| self.d.clone()))
    }
}

/// Creates a `NetTools` throughput watcher with a fake throughput backend.
fn mk_factory(v: Vec<HashMap<String, InterfaceCounters>>) -> Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync> {
    Arc::new(move |_, cfg| {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_millis(10)))
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
        s.set_throughput_backend(SeqTp::new(v.clone()));
        s
    })
}

#[test]
fn listener_id_without_tag() {
    assert_eq!(NetThroughputSensor::new("sid".to_string(), mk_cfg(None, false, 10)).listener_id_with_tag(), "net.throughput");
}

#[test]
fn listener_id_with_tag() {
    assert_eq!(NetThroughputSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).listener_id_with_tag(), "net.throughput@car");
}

#[test]
fn build_mask_is_throughput_updated() {
    let m = NetThroughputSensor::new("sid".to_string(), mk_cfg(None, false, 10)).build_mask();
    assert!(m.contains(nettools::events::NetToolsMask::THROUGHPUT_UPDATED));
}

#[test]
fn make_eid_with_tag() {
    assert_eq!(NetThroughputSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).make_eid("eth0"), "sid|net.throughput@car|updated@eth0|0");
}

#[tokio::test]
async fn recv_once_emits_throughput_updated_envelope() {
    let s = NetThroughputSensor::with_factory(
        "sid".to_string(),
        mk_cfg(Some("car"), false, 10),
        mk_factory(vec![
            HashMap::from([("eth0".to_string(), mk_cnt("eth0", 1000, 10, 2000, 20))]),
            HashMap::from([("eth0".to_string(), mk_cnt("eth0", 3000, 30, 5000, 50))]),
        ]),
    );
    let v = s.recv_once(Duration::from_millis(150)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.throughput@car|updated@eth0|0");
    assert_eq!(v["listener"], "net.throughput");
    assert_eq!(v["data"]["action"], "updated");
    assert_eq!(v["data"]["sample"]["iface"], "eth0");
    assert_eq!(v["data"]["sample"]["rx_bytes_per_sec"], 2000000);
    assert_eq!(v["data"]["sample"]["tx_packets_per_sec"], 30000);
}
