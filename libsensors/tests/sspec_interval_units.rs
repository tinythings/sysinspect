mod sspec_interval_units_test {
    use libsensors::sspec::SensorSpec;
    use std::{str::FromStr, time::Duration};

    fn one_sensor(unit: &str, min: u64, max: u64) -> String {
        format!(
            r#"
sensors:
  interval:
    min: {min}
    max: {max}
    unit: {unit}
  a:
    listener: sys.filesystem
    args: {{ path: /tmp }}
"#
        )
    }

    #[test]
    fn unit_aliases_are_respected() {
        let s_ms = SensorSpec::from_str(&one_sensor("ms", 7, 7)).unwrap();
        assert_eq!(s_ms.interval(), Duration::from_millis(7));

        let s_sec = SensorSpec::from_str(&one_sensor("sec", 2, 2)).unwrap();
        assert_eq!(s_sec.interval(), Duration::from_secs(2));

        let s_min = SensorSpec::from_str(&one_sensor("min", 3, 3)).unwrap();
        assert_eq!(s_min.interval(), Duration::from_secs(180));

        let s_hr = SensorSpec::from_str(&one_sensor("hr", 1, 1)).unwrap();
        assert_eq!(s_hr.interval(), Duration::from_secs(3600));
    }

    #[test]
    fn unknown_unit_defaults_to_seconds() {
        let s = SensorSpec::from_str(&one_sensor("nonsense", 4, 4)).unwrap();
        assert_eq!(s.interval(), Duration::from_secs(4));
    }

    #[test]
    fn range_swaps_and_clamps_to_at_least_one() {
        let s = SensorSpec::from_str(&one_sensor("seconds", 0, 0)).unwrap();
        assert_eq!(s.interval(), Duration::from_secs(1));

        let swapped = SensorSpec::from_str(&one_sensor("seconds", 9, 3)).unwrap();
        let got = swapped.interval().as_secs();
        assert!((3..=9).contains(&got));
    }

    #[test]
    fn items_injection_keeps_explicit_intervals_unchanged() {
        let y = r#"
sensors:
  interval:
    min: 10
    max: 10
    unit: seconds
  explicit:
    listener: sys.filesystem
    interval:
      secs: 2
      nanos: 0
    args: { path: /tmp/a }
  inherited:
    listener: sys.filesystem
    args: { path: /tmp/b }
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        let once = spec.items();
        let twice = spec.items();

        assert_eq!(once.get("explicit").unwrap().interval(), Some(Duration::from_secs(2)));
        assert_eq!(once.get("inherited").unwrap().interval(), Some(Duration::from_secs(10)));
        assert_eq!(once.get("inherited").unwrap().interval(), twice.get("inherited").unwrap().interval());
    }
}
