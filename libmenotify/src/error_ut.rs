use crate::MeNotifyError;

#[test]
fn menotify_error_converts_to_sysinspect_error() {
    let err: libcommon::SysinspectError = MeNotifyError::MissingModule("menotify".to_string()).into();
    assert_eq!(err.to_string(), "Error loading module: MeNotify: listener 'menotify' is missing a module name");
}
