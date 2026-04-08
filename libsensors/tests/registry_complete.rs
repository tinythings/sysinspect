mod registry_complete_test {
    use libsensors::sensors;
    use libsensors::sensors::SensorCtx;
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
    fn sys_proc_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("sys.proc");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn sys_mount_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("sys.mount");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_packet_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.packet");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_hostname_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.hostname");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_route_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.route");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_wifi_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.wifi");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_throughput_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.throughput");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }

    #[test]
    fn net_health_is_registered() {
        sensors::init_registry();
        let (sid, cfg) = cfg_for("net.health");
        let listener = cfg.listener().to_string();
        assert!(sensors::init_sensor(&listener, sid, cfg, SensorCtx::default()).is_some());
    }
}
