use super::modfinder::ModCall;
use crate::{cfg::mmconf::MinionConfig, inspector::SysInspectRunner};
use serde_json::json;

fn init_runner() {
    let mut cfg = MinionConfig::default();
    cfg.set_root_dir("/srv/sysinspect");
    cfg.set_sharelib_path("/opt/sysinspect/share");
    let _ = SysInspectRunner::new(&cfg);
}

#[test]
fn modcall_protocol_payload_contains_contract_sections() {
    init_runner();

    let mut call = ModCall::default();
    call.add_kwargs("name".to_string(), serde_yaml::to_value("Germany").unwrap_or_default());
    call.add_opt("lines".to_string());

    let payload: serde_json::Value =
        serde_json::from_str(&call.params_json_for_test()).unwrap_or_else(|err| panic!("failed to parse ModCall params JSON: {err}"));

    assert_eq!(payload.pointer("/arguments/name"), Some(&json!("Germany")));
    assert_eq!(payload.pointer("/options/0"), Some(&json!("lines")));
    assert!(payload.get("config").is_some());
    assert!(payload.get("host").is_some());
    assert_eq!(payload.pointer("/host/paths/sharelib"), Some(&json!("/opt/sysinspect/share")));
}

#[test]
fn modcall_protocol_payload_keeps_config_and_host_when_args_are_empty() {
    init_runner();

    let payload: serde_json::Value =
        serde_json::from_str(&ModCall::default().params_json_for_test()).unwrap_or_else(|err| panic!("failed to parse ModCall params JSON: {err}"));

    assert!(payload.get("arguments").is_none());
    assert!(payload.get("options").is_none());
    assert!(payload.get("config").is_some());
    assert!(payload.get("host").is_some());
    assert!(payload.pointer("/host/traits").and_then(|v| v.as_object()).is_some());
}
