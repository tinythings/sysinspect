use crate::MeNotifyRuntime;
use std::{fs, path::Path};

#[test]
fn resolves_script_path_from_listener_and_env_root() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    unsafe {
        std::env::set_var("SYSINSPECT_SHARELIB_ROOT", tmp.path());
    }
    let runtime = MeNotifyRuntime::new("demo".to_string(), "menotify.sample".to_string());
    unsafe {
        std::env::remove_var("SYSINSPECT_SHARELIB_ROOT");
    }

    assert_eq!(runtime.script_root(), Path::new(tmp.path()).join("lib/sensors/lua54"));
    assert_eq!(runtime.site_root(), Path::new(tmp.path()).join("lib/sensors/lua54/site-lua"));
    assert_eq!(
        runtime.script_path().expect("script path should resolve"),
        Path::new(tmp.path()).join("lib/sensors/lua54/sample.lua")
    );
}

#[test]
fn require_script_accepts_existing_file() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(root.join("demo.lua"), "return {}\n").expect("script file should be written");

    unsafe {
        std::env::set_var("SYSINSPECT_SHARELIB_ROOT", tmp.path());
    }
    let runtime = MeNotifyRuntime::new("demo".to_string(), "menotify.demo".to_string());
    unsafe {
        std::env::remove_var("SYSINSPECT_SHARELIB_ROOT");
    }

    assert_eq!(runtime.require_script().expect("script should exist"), root.join("demo.lua"));
}

#[test]
fn listener_without_module_stays_invalid() {
    let runtime = MeNotifyRuntime::new("demo".to_string(), "menotify".to_string());
    assert!(runtime.module_name().is_none());
    assert!(runtime.script_path().is_err());
}
