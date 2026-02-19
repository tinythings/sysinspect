mod registry_test {
    use libsensors::sensors;
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    #[test]
    fn test_registry_has_fsnotify_after_init() {
        sensors::init_registry();

        // should be able to create sensor by listener id
        let y = r#"
sensors:
  ssh-conf:
    listener: fsnotify
    opts: [changed]
    args:
      path: /tmp
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        let items = spec.items();
        let (sid, cfg) = items.iter().next().unwrap();

        let s = sensors::init_sensor(cfg.listener(), sid.to_string(), cfg.clone());
        assert!(s.is_some(), "fsnotify must be registered");
    }

    #[test]
    fn test_registry_unknown_listener_returns_none() {
        sensors::init_registry();

        let y = r#"
sensors:
  x:
    listener: does-not-exist
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        let items = spec.items();
        let (sid, cfg) = items.iter().next().unwrap();

        let s = sensors::init_sensor(cfg.listener(), sid.to_string(), cfg.clone());
        assert!(s.is_none(), "unknown listener must return None");
    }

    #[test]
    fn test_registry_init_is_idempotent() {
        sensors::init_registry();
        let n1 = sensors::REGISTRY.len();
        sensors::init_registry();
        let n2 = sensors::REGISTRY.len();
        assert_eq!(n1, n2);
        assert!(n1 >= 1);
    }
}
