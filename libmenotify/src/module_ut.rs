use crate::{MeNotifyError, MeNotifyModuleRef};
use std::path::Path;

#[test]
fn parses_module_listener() {
    let module = MeNotifyModuleRef::new("menotify.jira").expect("listener should parse");
    assert_eq!(module.listener(), "menotify.jira");
    assert_eq!(module.module(), "jira");
    assert_eq!(module.script_path(Path::new("/usr/share/sysinspect/lib/sensors/lua54")), Path::new("/usr/share/sysinspect/lib/sensors/lua54/jira.lua"));
}

#[test]
fn rejects_missing_module() {
    assert_eq!(
        MeNotifyModuleRef::new("menotify").expect_err("listener should fail"),
        MeNotifyError::MissingModule("menotify".to_string())
    );
}

#[test]
fn rejects_wrong_listener_family() {
    assert_eq!(
        MeNotifyModuleRef::new("fsnotify.foo").expect_err("listener should fail"),
        MeNotifyError::InvalidListener("fsnotify.foo".to_string())
    );
}
