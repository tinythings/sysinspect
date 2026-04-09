use crate::{
    sensors::{net_wifi::NetWifiSensor, sensor::Sensor},
    sspec::SensorConf,
};
use nettools::{NetTools, NetToolsConfig, WifiBackend, events::WifiDetails};
use serde_json::{from_value, json};
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(target_os = "linux")]
fn recv_wait() -> Duration {
    Duration::from_millis(150)
}

#[cfg(not(target_os = "linux"))]
fn recv_wait() -> Duration {
    Duration::from_secs(1)
}

/// Returns a `net.wifi` sensor configuration for tests.
fn mk_cfg(opts: Vec<&str>, tag: Option<&str>, lock: bool, ms: u64) -> SensorConf {
    from_value(json!({
        "listener": "net.wifi",
        "tag": tag,
        "opts": opts,
        "interval": {"secs": 0, "nanos": ms * 1_000_000},
        "args": {
            "locked": lock
        }
    }))
    .unwrap()
}

/// Creates a simple Wi-Fi state for tests.
fn mk_wifi(ifc: &str, conn: bool, qual: f32, sig: f32, noise: f32, ssid: Option<&str>, bssid: Option<&str>) -> WifiDetails {
    WifiDetails {
        iface: ifc.to_string(),
        connected: conn,
        link_quality: qual,
        signal_level_dbm: sig,
        noise_level_dbm: noise,
        ssid: ssid.map(str::to_string),
        bssid: bssid.map(str::to_string),
    }
}

/// Provides a deterministic Wi-Fi snapshot sequence for `NetTools` tests.
struct SeqWifi {
    q: Mutex<VecDeque<HashMap<String, WifiDetails>>>,
    d: HashMap<String, WifiDetails>,
}

impl SeqWifi {
    /// Creates a backend that returns queued Wi-Fi snapshots and then keeps the
    /// last one stable.
    fn new(v: Vec<HashMap<String, WifiDetails>>) -> Self {
        Self { d: v.last().cloned().unwrap_or_default(), q: Mutex::new(v.into()) }
    }
}

impl WifiBackend for SeqWifi {
    /// Returns the next queued Wi-Fi snapshot or the last stable snapshot.
    fn list(&self) -> io::Result<HashMap<String, WifiDetails>> {
        Ok(self.q.lock().unwrap().pop_front().unwrap_or_else(|| self.d.clone()))
    }
}

/// Creates a `NetTools` Wi-Fi watcher with a fake Wi-Fi backend.
fn mk_factory(v: Vec<HashMap<String, WifiDetails>>) -> Arc<dyn Fn(String, SensorConf) -> NetTools + Send + Sync> {
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
                .throughput(false)
                .wifi(true),
        ));
        s.set_wifi_backend(SeqWifi::new(v.clone()));
        s
    })
}

#[test]
fn listener_id_without_tag() {
    assert_eq!(NetWifiSensor::new("sid".to_string(), mk_cfg(vec![], None, false, 10)).listener_id_with_tag(), "net.wifi");
}

#[test]
fn listener_id_with_tag() {
    assert_eq!(NetWifiSensor::new("sid".to_string(), mk_cfg(vec![], Some("car"), false, 10)).listener_id_with_tag(), "net.wifi@car");
}

#[test]
fn build_mask_defaults_to_all_wifi_events() {
    let m = NetWifiSensor::new("sid".to_string(), mk_cfg(vec![], None, false, 10)).build_mask();
    assert!(m.contains(nettools::events::NetToolsMask::WIFI_ADDED));
    assert!(m.contains(nettools::events::NetToolsMask::WIFI_REMOVED));
    assert!(m.contains(nettools::events::NetToolsMask::WIFI_CHANGED));
}

#[test]
fn build_mask_respects_opts() {
    let m = NetWifiSensor::new("sid".to_string(), mk_cfg(vec!["changed"], None, false, 10)).build_mask();
    assert!(m.contains(nettools::events::NetToolsMask::WIFI_CHANGED));
    assert!(!m.contains(nettools::events::NetToolsMask::WIFI_ADDED));
}

#[test]
fn make_eid_with_tag() {
    assert_eq!(
        NetWifiSensor::new("sid".to_string(), mk_cfg(vec![], Some("car"), false, 10)).make_eid("connected", "wlan0"),
        "sid|net.wifi@car|connected@wlan0|0"
    );
}

#[tokio::test]
async fn recv_once_emits_wifi_changed_envelope() {
    let s = NetWifiSensor::with_factory(
        "sid".to_string(),
        mk_cfg(vec!["changed"], Some("car"), false, 10),
        mk_factory(vec![
            HashMap::from([("wlan0".to_string(), mk_wifi("wlan0", true, 42.0, -61.0, -95.0, Some("old"), Some("aa")))]),
            HashMap::from([("wlan0".to_string(), mk_wifi("wlan0", true, 45.0, -58.0, -92.0, Some("new"), Some("bb")))]),
        ]),
    );
    let v = s.recv_once(recv_wait()).await.unwrap();

    assert_eq!(v["eid"], "sid|net.wifi@car|changed@wlan0|0");
    assert_eq!(v["listener"], "net.wifi");
    assert_eq!(v["data"]["action"], "changed");
    assert_eq!(v["data"]["old"]["ssid"], "old");
    assert_eq!(v["data"]["new"]["ssid"], "new");
}

#[tokio::test]
async fn recv_once_emits_wifi_connected_envelope() {
    let s = NetWifiSensor::with_factory(
        "sid".to_string(),
        mk_cfg(vec!["connected"], None, false, 10),
        mk_factory(vec![
            HashMap::new(),
            HashMap::from([("wlan0".to_string(), mk_wifi("wlan0", true, 40.0, -60.0, -90.0, Some("car-net"), Some("aa")))]),
        ]),
    );
    let v = s.recv_once(Duration::from_millis(150)).await.unwrap();

    assert_eq!(v["eid"], "sid|net.wifi|connected@wlan0|0");
    assert_eq!(v["listener"], "net.wifi");
    assert_eq!(v["data"]["action"], "connected");
    assert_eq!(v["data"]["wifi"]["ssid"], "car-net");
}
