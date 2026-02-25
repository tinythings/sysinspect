use super::procnotify::ProcessSensor;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::sensor::Sensor;
    use crate::sspec::SensorConf;
    use procdog::events::{ProcDogEvent, ProcDogMask};
    use serde_json::{from_value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn mk_cfg(process: Option<&str>, opts: &[&str], tag: Option<&str>) -> SensorConf {
        from_value(json!({
            "listener": "procnotify",
            "tag": tag,
            "opts": opts,
            "args": {
                "process": process
            }
        }))
        .unwrap()
    }

    #[test]
    fn event_to_json_appeared() {
        let v = ProcessSensor::event_to_json(ProcDogEvent::Appeared { name: "sleep".into(), pid: 42 });

        assert_eq!(v["action"], "appeared");
        assert_eq!(v["process"], "sleep");
        assert_eq!(v["pid"], 42);
    }

    #[test]
    fn event_to_json_disappeared() {
        let v = ProcessSensor::event_to_json(ProcDogEvent::Disappeared { name: "sleep".into(), pid: 42 });

        assert_eq!(v["action"], "disappeared");
        assert_eq!(v["process"], "sleep");
        assert_eq!(v["pid"], 42);
    }

    #[test]
    fn event_to_json_missing() {
        let v = ProcessSensor::event_to_json(ProcDogEvent::Missing { name: "sleep".into() });

        assert_eq!(v["action"], "missing");
        assert_eq!(v["process"], "sleep");
        assert!(v.get("pid").is_none());
    }

    #[test]
    fn build_mask_with_missing_opt() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &["missing"], None));

        let m = s.build_mask();

        assert!(m.contains(ProcDogMask::MISSING));
        assert!(!m.contains(ProcDogMask::APPEARED));
        assert!(!m.contains(ProcDogMask::DISAPPEARED));
    }

    #[test]
    fn build_mask_combined_opts() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &["appeared", "missing"], None));

        let m = s.build_mask();

        assert!(m.contains(ProcDogMask::APPEARED));
        assert!(m.contains(ProcDogMask::MISSING));
        assert!(!m.contains(ProcDogMask::DISAPPEARED));
    }

    #[test]
    fn build_mask_defaults_to_both() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &[], None));
        let m = s.build_mask();

        assert!(m.contains(ProcDogMask::APPEARED));
        assert!(m.contains(ProcDogMask::DISAPPEARED));
    }

    #[test]
    fn build_mask_respects_opts() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &["appeared"], None));
        let m = s.build_mask();

        assert!(m.contains(ProcDogMask::APPEARED));
        assert!(!m.contains(ProcDogMask::DISAPPEARED));
    }

    #[test]
    fn make_eid_without_tag() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &["appeared"], None));
        let eid = s.make_eid("appeared", "sleep");

        assert_eq!(eid, "SID|procnotify|appeared@sleep|0");
    }

    #[test]
    fn make_eid_with_tag() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("sleep"), &["appeared"], Some("grim")));
        let eid = s.make_eid("appeared", "sleep");

        assert_eq!(eid, "SID|procnotify@grim|appeared@sleep|0");
    }

    #[tokio::test]
    async fn run_returns_early_when_process_missing_and_does_not_emit() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(None, &["appeared"], None));

        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn run_returns_early_when_process_empty_and_does_not_emit() {
        let s = ProcessSensor::new("SID".into(), mk_cfg(Some("   "), &["appeared"], None));

        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
