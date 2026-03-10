#[cfg(test)]
mod tests {
    use crate::sensors::{netnotify::NetNotifySensor, sensor::Sensor};
    use crate::sspec::SensorConf;
    use serde_json::{from_value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn mk_cfg(patterns: Option<Vec<&str>>, ignore: Vec<&str>, opts: Vec<&str>, tag: Option<&str>) -> SensorConf {
        let patterns_json = patterns.map(|v| v.into_iter().map(|s| json!(s)).collect::<Vec<_>>());

        from_value(json!({
            "listener": "netnotify",
            "tag": tag,
            "opts": opts,
            "args": {
                "patterns": patterns_json,
                "ignore": ignore,
                "locked": false,
                "dns-ttl": 60
            }
        }))
        .unwrap()
    }

    #[test]
    fn build_mask_defaults_to_both() {
        let s = NetNotifySensor::new("SID".into(), mk_cfg(Some(vec!["*"]), vec![], vec![], None));
        let m = s.build_mask();
        assert!(m.contains(netpacket::events::NetNotifyMask::OPENED));
        assert!(m.contains(netpacket::events::NetNotifyMask::CLOSED));
    }

    #[test]
    fn build_mask_respects_opts() {
        let s = NetNotifySensor::new("SID".into(), mk_cfg(Some(vec!["*"]), vec![], vec!["opened"], None));
        let m = s.build_mask();
        assert!(m.contains(netpacket::events::NetNotifyMask::OPENED));
        assert!(!m.contains(netpacket::events::NetNotifyMask::CLOSED));
    }

    #[test]
    fn pattern_needs_dns_host_glob_true() {
        assert!(NetNotifySensor::pattern_needs_dns("*.1e100.net"));
        assert!(NetNotifySensor::pattern_needs_dns("google.com"));
    }

    #[test]
    fn pattern_needs_dns_ip_false() {
        assert!(!NetNotifySensor::pattern_needs_dns("136.2.168.192"));
        assert!(!NetNotifySensor::pattern_needs_dns("136.2.168.192:443"));
    }

    #[tokio::test]
    async fn run_returns_early_when_patterns_missing_and_does_not_emit() {
        // patterns=None => missing args.patterns
        let s = NetNotifySensor::new("SID".into(), mk_cfg(None, vec![], vec!["opened"], None));

        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn run_returns_early_when_patterns_empty_and_does_not_emit() {
        // patterns=[]
        let s = NetNotifySensor::new("SID".into(), mk_cfg(Some(vec![]), vec![], vec!["opened"], None));

        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
