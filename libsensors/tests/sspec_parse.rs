mod sspec_parse_test {
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    #[test]
    fn test_parse_minimal_sensor() {
        let y = r#"
sensors:
  ssh-conf:
    listener: file
"#;

        let spec = SensorSpec::from_str(y).unwrap();
        let items = spec.items();
        assert_eq!(items.len(), 1);

        let c = items.get("ssh-conf").unwrap();
        assert_eq!(c.listener(), "file");
        assert!(c.description().is_none());
        assert!(c.opts().is_empty());
        assert!(c.event().is_none());
    }

    #[test]
    fn test_parse_interval() {
        let y = r#"
sensors:
  interval:
    min: 3
    max: 10
    unit: seconds

  a:
    listener: file
"#;

        let spec = SensorSpec::from_str(y).unwrap();
        let ir = spec.interval().unwrap();
        assert_eq!(ir.min, 3);
        assert_eq!(ir.max, 10);
        assert_eq!(ir.unit, "seconds");
    }

    #[test]
    fn test_parse_opts_args_event_profile() {
        let y = r#"
sensors:
  ssh-conf:
    profile: [default, system]
    description: Watches SSH config
    listener: file
    opts: [changed, deleted]
    args:
      path: /etc/ssh/ssh_config
      interval: 5
      unit: second
    event: ssh-conf/file/changed/0
"#;

        let spec = SensorSpec::from_str(y).unwrap();
        let c = spec.items().get("ssh-conf").unwrap();

        assert_eq!(c.listener(), "file");
        assert_eq!(c.description().unwrap(), "Watches SSH config");
        assert_eq!(c.opts(), &vec!["changed".to_string(), "deleted".to_string()]);
        assert_eq!(c.event().unwrap(), "ssh-conf/file/changed/0");

        // profile() currently returns normalized Vec<String>
        let p = c.profile();
        assert!(p.contains(&"default".to_string()));
        assert!(p.contains(&"system".to_string()));

        // args is YAML value, just sanity check it exists
        let args = c.args();
        assert!(args.is_mapping());
    }

    #[test]
    fn test_interval_injection_is_random_and_stable_per_spec() {
        use libsensors::sspec::SensorSpec;
        use std::{collections::HashSet, str::FromStr};

        // Wide range -> collisions become hilariously unlikely.
        let y = r#"
sensors:
  interval:
    min: 1
    max: 1000
    unit: seconds

  a:
    listener: file
    args:
      path: /tmp/whatever
"#;

        let mut seen = HashSet::new();

        for _ in 0..100 {
            let mut spec = SensorSpec::from_str(y).unwrap();

            let first = spec.items();
            let a1 = first.get("a").unwrap();

            let seen_ivl = a1.interval(); // e.g. returns u64 / usize / i64 etc.
            seen.insert(seen_ivl);

            let second = spec.items();
            let a2 = second.get("a").unwrap();
            let coming_ivl = a2.interval();

            assert_eq!(seen_ivl, coming_ivl, "interval changed after injection; injected flag isn't working");
        }

        assert!(seen.len() >= 2, "interval injection doesn't look random: got only one value across runs: {:?}", seen);
    }
}
