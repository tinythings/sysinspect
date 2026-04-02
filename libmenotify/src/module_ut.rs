use crate::{MeNotifyError, MeNotifyModuleRef};
use std::path::Path;

#[test]
fn parses_module_listener() {
    let module = MeNotifyModuleRef::new("menotify.jira").expect("listener should parse");
    assert_eq!(module.listener(), "menotify.jira");
    assert_eq!(module.module(), "jira");
    assert_eq!(module.script_path(Path::new("/usr/share/sysinspect/lib/sensors/lua")), Path::new("/usr/share/sysinspect/lib/sensors/lua/jira.lua"));
}

#[test]
fn rejects_missing_module() {
    assert!(matches!(
        MeNotifyModuleRef::new("menotify").expect_err("listener should fail"),
        MeNotifyError::MissingModule(listener) if listener == "menotify"
    ));
}

#[test]
fn rejects_wrong_listener_family() {
    assert!(matches!(
        MeNotifyModuleRef::new("sys.filesystem.foo").expect_err("listener should fail"),
        MeNotifyError::InvalidListener(listener) if listener == "sys.filesystem.foo"
    ));
}
