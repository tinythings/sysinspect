mod spec_items_test {
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    #[test]
    fn test_items_exposes_sensor_ids() {
        let y = r#"
sensors:
  b:
    listener: fsnotify
    args: { path: /tmp }
  a:
    listener: fsnotify
    args: { path: /etc }
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        assert!(spec.items().contains_key("a"));
        assert!(spec.items().contains_key("b"));
    }

    #[test]
    fn test_missing_sensors_key_is_error() {
        let y = r#"
not_sensors:
  a:
    listener: fsnotify
"#;

        let r = SensorSpec::from_str(y);
        assert!(r.is_err());
    }
}
