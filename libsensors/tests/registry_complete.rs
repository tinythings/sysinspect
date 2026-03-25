mod registry_complete_test {
    use libsensors::sensors;
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    fn cfg_for(listener: &str) -> (String, libsensors::sspec::SensorConf) {
        let y = format!(
            r#"
sensors:
  x:
    listener: {listener}
"#
        );
        let mut spec = SensorSpec::from_str(&y).unwrap();
        let items = spec.items();
        let (sid, cfg) = items.iter().next().unwrap();
        (sid.to_string(), cfg.clone())
    }

    #[test]
    fn procnotify_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("procnotify");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg).is_some());
    }

    #[test]
    fn mountnotify_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("mountnotify");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg).is_some());
    }

    #[test]
    fn netnotify_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("netnotify");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg).is_some());
    }

    #[test]
    fn net_hostname_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.hostname");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg).is_some());
    }

    #[test]
    fn net_route_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.route");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg).is_some());
    }
}
