use super::host::build_runtime_host_context;
use crate::{
    cfg::mmconf::MinionConfig,
    traits::{SYS_NET_HOSTNAME, SYS_OS_NAME, systraits::SystemTraits},
};
use serde_json::json;

#[test]
fn runtime_host_context_contains_stable_sections() {
    let mut cfg = MinionConfig::default();
    cfg.set_root_dir("/srv/sysinspect");
    cfg.set_sharelib_path("/opt/sysinspect/share");

    let traits = SystemTraits::from_map([
        (SYS_OS_NAME.to_string(), json!("NetBSD")),
        (SYS_NET_HOSTNAME.to_string(), json!("minion1")),
        ("custom.operator.label".to_string(), json!("special")),
    ]);

    let host = serde_json::to_value(build_runtime_host_context(&cfg, &traits)).unwrap_or_default();

    assert_eq!(host.pointer("/traits/system.os.name"), Some(&json!("NetBSD")));
    assert_eq!(host.pointer("/traits/system.hostname"), Some(&json!("minion1")));
    assert_eq!(host.pointer("/traits/custom.operator.label"), Some(&json!("special")));
    assert_eq!(host.pointer("/paths/sharelib"), Some(&json!("/opt/sysinspect/share")));
    assert_eq!(host.pointer("/paths/root"), Some(&json!("/srv/sysinspect")));
    assert_eq!(host.pointer("/paths/models"), Some(&json!("/srv/sysinspect/models")));
    assert_eq!(host.pointer("/capabilities/packagekit"), Some(&json!(false)));
    assert!(host.get("sys").is_none());
}
