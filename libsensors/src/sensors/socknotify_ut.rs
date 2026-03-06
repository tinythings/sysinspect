#[cfg(test)]
mod socknotify_ut {
    use crate::sensors::{sensor::Sensor, socknotify::SockTraySensor};
    use crate::sspec::SensorConf;

    fn mk_cfg(opts: Vec<&str>) -> SensorConf {
        serde_yaml::from_str(&format!(
            r#"
listener: socknotify
opts: [{}]
"#,
            opts.into_iter().map(|s| format!(r#""{s}""#)).collect::<Vec<_>>().join(", ")
        ))
        .unwrap()
    }

    #[test]
    fn defaults_to_opened_and_closed() {
        let s = SockTraySensor::new("SID".into(), mk_cfg(vec![]));
        let m = s.build_mask();
        assert!(m.contains(socktray::events::SockTrayMask::OPENED));
        assert!(m.contains(socktray::events::SockTrayMask::CLOSED));
    }

    #[test]
    fn parses_specific_opts() {
        let s = SockTraySensor::new("SID".into(), mk_cfg(vec!["opened"]));
        let m = s.build_mask();
        assert!(m.contains(socktray::events::SockTrayMask::OPENED));
        assert!(!m.contains(socktray::events::SockTrayMask::CLOSED));
    }
}
