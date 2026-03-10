mod argparse_args_test {
    use libsensors::argparse::SensorArgs;
    use libsensors::sspec::SensorConf;
    use serde_json::{from_value, json};
    use std::time::Duration;

    fn mk_cfg(args: serde_json::Value) -> SensorConf {
        from_value(json!({
            "listener": "dummy",
            "args": args
        }))
        .unwrap()
    }

    #[test]
    fn arg_str_array_trims_and_drops_empty_values() {
        let cfg = mk_cfg(json!({
            "patterns": ["  a  ", "", "   ", "b", 123]
        }));

        let out = cfg.arg_str_array("patterns").unwrap();
        assert_eq!(out, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn arg_duration_parses_and_rejects_invalid_values() {
        let ok = mk_cfg(json!({ "dns-ttl": "90s" }));
        assert_eq!(ok.arg_duration("dns-ttl"), Some(Duration::from_secs(90)));

        let bad = mk_cfg(json!({ "dns-ttl": "not-a-duration" }));
        assert!(bad.arg_duration("dns-ttl").is_none());
    }

    #[test]
    fn arg_u64_casts_negative_i64_current_behavior() {
        let cfg = mk_cfg(json!({ "n": -1 }));
        assert_eq!(cfg.arg_u64("n"), Some(u64::MAX));
    }
}
