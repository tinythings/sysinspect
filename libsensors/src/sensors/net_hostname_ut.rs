use crate::{
    sensors::{
        net_hostname::NetHostnameSensor,
        sensor::Sensor,
    },
    sspec::SensorConf,
};
use async_trait::async_trait;
use nettools::{
    HostnameBackend, NetTools, NetToolsConfig,
    events::NetToolsEvent,
};
use omnitrace_core::callbacks::Callback;
use serde_json::{from_value, json};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Returns a `net.hostname` sensor configuration for tests.
fn mk_cfg(tag: Option<&str>, lock: bool, ms: u64) -> SensorConf {
    from_value(json!({
        "listener": "net.hostname",
        "tag": tag,
        "interval": {"secs": 0, "nanos": ms * 1_000_000},
        "args": {
            "locked": lock
        }
    }))
    .unwrap()
}

/// Provides a deterministic hostname sequence for `NetTools` tests.
struct SeqHost {
    q: Mutex<VecDeque<String>>,
    d: String,
}

impl SeqHost {
    /// Creates a hostname backend that returns the queued values in order and
    /// then keeps returning the last value.
    fn new(v: Vec<&str>) -> Self {
        Self {
            d: v.last().unwrap_or(&"host").to_string(),
            q: Mutex::new(v.into_iter().map(str::to_string).collect()),
        }
    }
}

impl HostnameBackend for SeqHost {
    /// Returns the next queued hostname or the last stable hostname.
    fn current(&self) -> io::Result<String> {
        Ok(self.q.lock().unwrap().pop_front().unwrap_or_else(|| self.d.clone()))
    }
}

/// Creates a `NetTools` hostname watcher with a fake hostname backend.
fn mk_factory(v: Vec<&str>) -> Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync> {
    let v = v.into_iter().map(str::to_string).collect::<Vec<_>>();
    Arc::new(move |_, cfg| {
        let mut s = NetTools::new(Some(
            NetToolsConfig::default()
                .pulse(cfg.interval().unwrap_or_else(|| Duration::from_millis(10)))
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
        s.set_hostname_backend(SeqHost::new(v.iter().map(String::as_str).collect()));
        s
    })
}

#[test]
fn listener_id_without_tag() {
    assert_eq!(NetHostnameSensor::new("sid".to_string(), mk_cfg(None, false, 10)).listener_id_with_tag(), "net.hostname");
}

#[test]
fn listener_id_with_tag() {
    assert_eq!(NetHostnameSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).listener_id_with_tag(), "net.hostname@car");
}

#[test]
fn make_eid_without_tag() {
    assert_eq!(NetHostnameSensor::new("sid".to_string(), mk_cfg(None, false, 10)).make_eid("new-host"), "sid|net.hostname|changed@new-host|0");
}

#[test]
fn make_eid_with_tag() {
    assert_eq!(NetHostnameSensor::new("sid".to_string(), mk_cfg(Some("car"), false, 10)).make_eid("new-host"), "sid|net.hostname@car|changed@new-host|0");
}

#[tokio::test]
async fn bridge_ignores_non_hostname_events() {
    struct Cb;

    #[async_trait]
    impl Callback<NetToolsEvent> for Cb {
        fn mask(&self) -> u64 {
            0
        }

        async fn call(&self, _: &NetToolsEvent) -> Option<serde_json::Value> {
            None
        }
    }

    assert!(Cb.call(&NetToolsEvent::RouteAdded {
        route: nettools::events::RouteEntry {
            family: nettools::events::RouteFamily::Inet,
            destination: "0.0.0.0/0".to_string(),
            gateway: "1.1.1.1".to_string(),
            iface: "eth0".to_string(),
        },
    })
    .await
    .is_none());
}

#[tokio::test]
async fn recv_once_emits_hostname_change_envelope() {
    let s = NetHostnameSensor::with_factory("sid".to_string(), mk_cfg(Some("car"), false, 10), mk_factory(vec!["old-host", "new-host"]));
    let v = s.recv_once(Duration::from_millis(150)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.hostname@car|changed@new-host|0");
    assert_eq!(v["sensor"], "sid");
    assert_eq!(v["listener"], "net.hostname");
    assert_eq!(v["data"]["action"], "changed");
    assert_eq!(v["data"]["old"], "old-host");
    assert_eq!(v["data"]["new"], "new-host");
}
