use crate::sensors::{ifacenotify::IfaceSensor, sensor::Sensor};
use crate::sspec::SensorConf;

fn mk_cfg(opts: Vec<&str>) -> SensorConf {
    serde_yaml::from_str(&format!(
        r#"
listener: net.iface
opts: [{}]
"#,
        opts.into_iter().map(|s| format!(r#""{s}""#)).collect::<Vec<_>>().join(", ")
    ))
    .unwrap()
}

#[test]
fn defaults_to_all_events() {
    let s = IfaceSensor::new("SID".into(), mk_cfg(vec![]));
    let m = s.build_mask();
    assert!(m.contains(iface::events::IfaceMask::IFACE_ADDED));
    assert!(m.contains(iface::events::IfaceMask::IFACE_REMOVED));
    assert!(m.contains(iface::events::IfaceMask::LINK_UP));
    assert!(m.contains(iface::events::IfaceMask::LINK_DOWN));
    assert!(m.contains(iface::events::IfaceMask::ADDR_ADDED));
    assert!(m.contains(iface::events::IfaceMask::ADDR_REMOVED));
}

#[test]
fn parses_specific_opts() {
    let s = IfaceSensor::new("SID".into(), mk_cfg(vec!["link-up", "addr-removed"]));
    let m = s.build_mask();
    assert!(m.contains(iface::events::IfaceMask::LINK_UP));
    assert!(m.contains(iface::events::IfaceMask::ADDR_REMOVED));
    assert!(!m.contains(iface::events::IfaceMask::IFACE_ADDED));
    assert!(!m.contains(iface::events::IfaceMask::LINK_DOWN));
}
