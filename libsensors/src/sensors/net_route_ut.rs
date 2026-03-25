use crate::{
    sensors::{
        net_route::NetRouteSensor,
        sensor::Sensor,
    },
    sspec::SensorConf,
};
use nettools::{
    NetTools, NetToolsConfig, RouteBackend,
    events::{RouteEntry, RouteFamily},
};
use serde_json::{from_value, json};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Returns a `net.route` sensor configuration for tests.
fn mk_cfg(opts: Vec<&str>, tag: Option<&str>, lock: bool, ms: u64) -> SensorConf {
    from_value(json!({
        "listener": "net.route",
        "tag": tag,
        "opts": opts,
        "interval": {"secs": 0, "nanos": ms * 1_000_000},
        "args": {
            "locked": lock
        }
    }))
    .unwrap()
}

/// Creates a simple route entry for tests.
fn mk_route(dst: &str, gw: &str, ifc: &str) -> RouteEntry {
    RouteEntry {
        family: RouteFamily::Inet,
        destination: dst.to_string(),
        gateway: gw.to_string(),
        iface: ifc.to_string(),
    }
}

/// Provides a deterministic route snapshot sequence for `NetTools` tests.
struct SeqRoute {
    q: Mutex<VecDeque<Vec<RouteEntry>>>,
    d: Vec<RouteEntry>,
}

impl SeqRoute {
    /// Creates a backend that returns queued route snapshots and then keeps the
    /// last one stable.
    fn new(v: Vec<Vec<RouteEntry>>) -> Self {
        Self {
            d: v.last().cloned().unwrap_or_default(),
            q: Mutex::new(v.into()),
        }
    }
}

impl RouteBackend for SeqRoute {
    /// Returns the next queued route snapshot or the last stable snapshot.
    fn list(&self) -> io::Result<Vec<RouteEntry>> {
        Ok(self.q.lock().unwrap().pop_front().unwrap_or_else(|| self.d.clone()))
    }
}

/// Creates a `NetTools` route watcher with a fake route backend.
fn mk_factory(v: Vec<Vec<RouteEntry>>) -> Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync> {
    Arc::new(move |_, cfg| {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_millis(10)))
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
        s.set_route_backend(SeqRoute::new(v.clone()));
        s
    })
}

#[test]
fn listener_id_without_tag() {
    assert_eq!(NetRouteSensor::new("sid".to_string(), mk_cfg(vec![], None, false, 10)).listener_id_with_tag(), "net.route");
}

#[test]
fn listener_id_with_tag() {
    assert_eq!(NetRouteSensor::new("sid".to_string(), mk_cfg(vec![], Some("car"), false, 10)).listener_id_with_tag(), "net.route@car");
}

#[test]
fn build_mask_defaults_to_all_route_events() {
    let m = NetRouteSensor::new("sid".to_string(), mk_cfg(vec![], None, false, 10)).build_mask();
    assert!(m.contains(nettools::events::NetToolsMask::ROUTE_ADDED));
    assert!(m.contains(nettools::events::NetToolsMask::ROUTE_REMOVED));
    assert!(m.contains(nettools::events::NetToolsMask::ROUTE_CHANGED));
    assert!(m.contains(nettools::events::NetToolsMask::DEFAULT_ROUTE_ADDED));
    assert!(m.contains(nettools::events::NetToolsMask::DEFAULT_ROUTE_REMOVED));
    assert!(m.contains(nettools::events::NetToolsMask::DEFAULT_ROUTE_CHANGED));
}

#[test]
fn build_mask_respects_opts() {
    let m = NetRouteSensor::new("sid".to_string(), mk_cfg(vec!["default-changed"], None, false, 10)).build_mask();
    assert!(m.contains(nettools::events::NetToolsMask::DEFAULT_ROUTE_CHANGED));
    assert!(!m.contains(nettools::events::NetToolsMask::ROUTE_ADDED));
}

#[test]
fn make_eid_with_tag() {
    assert_eq!(
        NetRouteSensor::new("sid".to_string(), mk_cfg(vec![], Some("car"), false, 10)).make_eid("route-added", "10.0.0.0/24"),
        "sid|net.route@car|route-added@10.0.0.0/24|0"
    );
}

#[tokio::test]
async fn recv_once_emits_route_changed_envelope() {
    let s = NetRouteSensor::with_factory(
        "sid".to_string(),
        mk_cfg(vec!["route-changed"], Some("car"), false, 10),
        mk_factory(vec![
            vec![mk_route("10.0.0.0/24", "10.0.0.1", "eth0")],
            vec![mk_route("10.0.0.0/24", "10.0.0.254", "eth1")],
        ]),
    );
    let v = s.recv_once(Duration::from_millis(150)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.route@car|route-changed@10.0.0.0/24|0");
    assert_eq!(v["listener"], "net.route");
    assert_eq!(v["data"]["action"], "route-changed");
    assert_eq!(v["data"]["old"]["gateway"], "10.0.0.1");
    assert_eq!(v["data"]["new"]["gateway"], "10.0.0.254");
}

#[tokio::test]
async fn recv_once_emits_default_route_changed_envelope() {
    let s = NetRouteSensor::with_factory(
        "sid".to_string(),
        mk_cfg(vec!["default-changed"], None, false, 10),
        mk_factory(vec![
            vec![mk_route("default", "10.0.0.1", "eth0")],
            vec![mk_route("default", "10.0.0.254", "eth1")],
        ]),
    );
    let v = s.recv_once(Duration::from_millis(150)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.route|default-changed@default|0");
    assert_eq!(v["listener"], "net.route");
    assert_eq!(v["data"]["action"], "default-changed");
    assert_eq!(v["data"]["old"]["gateway"], "10.0.0.1");
    assert_eq!(v["data"]["new"]["gateway"], "10.0.0.254");
}
