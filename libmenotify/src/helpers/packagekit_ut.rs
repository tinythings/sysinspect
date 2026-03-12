use super::packagekit::PackageKitPackage;
use crate::{MeNotifyContext, MeNotifyEventBuilder, MeNotifyProgram, MeNotifyRunner, MeNotifyRuntime};
use std::{fs, sync::Mutex, time::Duration};

#[test]
fn tick_runner_exposes_packagekit_available_helper() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
    tick = function(ctx)
        ctx.emit({ available = packagekit.available() })
    end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new(
            "demo",
            "menotify.demo",
            "demo",
            &[],
            &serde_yaml::from_str("{}\n").expect("yaml should parse"),
            Some(Duration::from_secs(1)),
        ),
    );
    let out = Mutex::new(Vec::new());

    runner
        .run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &MeNotifyEventBuilder::new("demo", "menotify.demo", None))
        .expect("tick with packagekit helper should succeed");

    let events = out.lock().expect("lock should work");
    assert!(events[0]["data"]["available"].is_boolean());
}

#[test]
fn installed_package_parser_rejects_non_installed_entries() {
    assert!(PackageKitPackage::from_signal(1, "cowsay;1.0;noarch;fedora", "Cowsay").is_none());
}

#[test]
fn installed_package_parser_accepts_installed_entries() {
    assert_eq!(
        PackageKitPackage::from_signal(1, "cowsay;1.0;noarch;installed:fedora", "Cowsay").expect("installed package should parse"),
        PackageKitPackage {
            info: 1,
            package_id: "cowsay;1.0;noarch;installed:fedora".to_string(),
            name: "cowsay".to_string(),
            version: "1.0".to_string(),
            arch: "noarch".to_string(),
            data: "installed:fedora".to_string(),
            summary: "Cowsay".to_string(),
        }
    );
}
