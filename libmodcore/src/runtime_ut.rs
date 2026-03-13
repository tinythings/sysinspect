use crate::runtime::{ModRequest, get_arg, get_arg_default, get_opt};
use libsysinspect::cfg::mmconf::DEFAULT_MODULES_SHARELIB;
use serde_json::json;

#[test]
fn mod_request_filters_runtime_internal_fields_from_public_sections() {
    let rq: ModRequest = serde_json::from_value(json!({
        "opts": ["lines", "rt.logs", "rt.list"],
        "args": {
            "name": "Germany",
            "count": 2,
            "rt.mod": "reader",
            "rt.man": true
        },
        "host": {
            "sys": { "hostname": { "short": "minion-a" } }
        },
        "custom.flag": true
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.options().len(), 1);
    assert_eq!(rq.options()[0].as_string(), Some("lines".to_string()));
    assert!(rq.has_option("rt.logs"));
    assert_eq!(rq.args().len(), 2);
    assert_eq!(rq.args().get("name").and_then(|v| v.as_string()), Some("Germany".to_string()));
    assert_eq!(rq.args_all().get("rt.mod").and_then(|v| v.as_string()), Some("reader".to_string()));
    assert_eq!(rq.host().pointer("/sys/hostname/short"), Some(&json!("minion-a")));
    assert_eq!(rq.ext().get("custom.flag"), Some(&json!(true)));
}

#[test]
fn mod_request_injects_sharelib_and_defaults_empty_host() {
    let rq: ModRequest = serde_json::from_value(json!({
        "args": { "name": "Germany" }
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.config().get("path.sharelib").and_then(|v| v.as_string()), Some(DEFAULT_MODULES_SHARELIB.to_string()));
    assert_eq!(rq.host(), json!({}));
}

#[test]
fn mod_request_helpers_keep_runtime_fields_addressable() {
    let rq: ModRequest = serde_json::from_value(json!({
        "opts": ["plain", "rt.logs"],
        "args": {
            "name": "Germany",
            "rt.mod": "hello",
            "rt.man": true
        }
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(get_arg(&rq, "name"), "Germany");
    assert_eq!(get_arg(&rq, "rt.man"), "true");
    assert_eq!(get_arg_default(&rq, "missing", "fallback"), "fallback");
    assert!(get_opt(&rq, "plain"));
    assert!(!get_opt(&rq, "rt.logs"));
}

#[test]
fn mod_request_accepts_arguments_and_options_legacy_shape() {
    let rq: ModRequest = serde_json::from_value(json!({
        "options": ["plain", "rt.logs"],
        "arguments": {
            "name": "Germany",
            "rt.mod": "hello"
        },
        "host": {
            "traits": {
                "system.hostname": "legacy-minion"
            }
        }
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.options()[0].as_string(), Some("plain".to_string()));
    assert_eq!(rq.args().get("name").and_then(|v| v.as_string()), Some("Germany".to_string()));
    assert_eq!(rq.args_all().get("rt.mod").and_then(|v| v.as_string()), Some("hello".to_string()));
    assert_eq!(rq.host().pointer("/traits/system.hostname"), Some(&json!("legacy-minion")));
}

#[test]
fn mod_request_preserves_partial_host_payload_without_failing() {
    let rq: ModRequest = serde_json::from_value(json!({
        "arguments": {
            "name": "Germany"
        },
        "host": {
            "paths": {
                "sharelib": "/srv/share"
            }
        }
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.args().get("name").and_then(|v| v.as_string()), Some("Germany".to_string()));
    assert_eq!(rq.host().pointer("/paths/sharelib"), Some(&json!("/srv/share")));
    assert_eq!(rq.host().pointer("/traits/system.hostname"), None);
}

#[test]
fn mod_request_accepts_explicit_ext_object() {
    let rq: ModRequest = serde_json::from_value(json!({
        "arguments": {
            "name": "Germany"
        },
        "ext": {
            "trace_id": "abc-123",
            "payload": {
                "mode": "demo"
            }
        }
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.args().get("name").and_then(|v| v.as_string()), Some("Germany".to_string()));
    assert_eq!(rq.ext().get("trace_id"), Some(&json!("abc-123")));
    assert_eq!(rq.ext().get("payload"), Some(&json!({"mode": "demo"})));
}

#[test]
fn mod_request_merges_explicit_ext_and_flat_passthrough_fields() {
    let rq: ModRequest = serde_json::from_value(json!({
        "args": {
            "name": "Germany"
        },
        "ext": {
            "trace_id": "nested",
            "payload": {
                "mode": "demo"
            }
        },
        "trace_id": "flat",
        "request_id": "req-42"
    }))
    .unwrap_or_else(|err| panic!("failed to parse ModRequest: {err}"));

    assert_eq!(rq.args().get("name").and_then(|v| v.as_string()), Some("Germany".to_string()));
    assert_eq!(rq.ext().get("trace_id"), Some(&json!("flat")));
    assert_eq!(rq.ext().get("request_id"), Some(&json!("req-42")));
    assert_eq!(rq.ext().get("payload"), Some(&json!({"mode": "demo"})));
}
