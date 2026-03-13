use crate::mpk::ModPakMetadata;
use std::path::PathBuf;

#[test]
fn runtime_dispatcher_names_are_reserved() {
    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/lua-runtime"), "runtime.lua");
    assert!(meta.validate_name_for_path().is_ok());

    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/not-a-runtime"), "runtime.lua");
    assert!(meta.validate_name_for_path().is_err());

    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/custom-module"), "lua.reader");
    assert!(meta.validate_name_for_path().is_err());
}
