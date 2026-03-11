use crate::MeNotifyEventBuilder;
use serde_json::json;

#[test]
fn event_builder_uses_defaults() {
    let ev = MeNotifyEventBuilder::new("sensor-a", "menotify.demo", None).build(json!({"hello":"world"}), None).expect("event should build");

    assert_eq!(ev["eid"], "sensor-a|menotify.demo|emitted@-|0");
    assert_eq!(ev["sensor"], "sensor-a");
    assert_eq!(ev["listener"], "menotify.demo");
    assert_eq!(ev["data"]["hello"], "world");
}

#[test]
fn event_builder_uses_meta_and_tag() {
    let ev = MeNotifyEventBuilder::new("sensor-a", "menotify.demo", Some("blue"))
        .build(json!({"hello":"world"}), Some(json!({"action":"created","key":"OPS-1"})))
        .expect("event should build");

    assert_eq!(ev["eid"], "sensor-a|menotify.demo@blue|created@OPS-1|0");
}

#[test]
fn event_builder_rejects_malformed_emit_meta() {
    let err = MeNotifyEventBuilder::new("sensor-a", "menotify.demo", None)
        .build(json!({"hello":"world"}), Some(json!({"action": 42})))
        .expect_err("event should reject malformed metadata");

    assert!(matches!(err, crate::MeNotifyError::InvalidEmitMeta(_)));
}
