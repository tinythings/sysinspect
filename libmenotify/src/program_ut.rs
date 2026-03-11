use crate::{MeNotifyEntrypoint, MeNotifyError, MeNotifyProgram, MeNotifyRuntime};
use std::fs;

#[test]
fn loads_tick_program() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  tick = function(_ctx) end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let program = MeNotifyProgram::new(&runtime).expect("program should load");
    assert_eq!(program.module_name(), "demo");
    assert_eq!(program.contract().entrypoint(), MeNotifyEntrypoint::Tick);
    assert_eq!(program.script_path(), root.join("demo.lua"));
}

#[test]
fn rejects_program_without_entrypoint() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(root.join("broken.lua"), "return {}\n").expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.broken".to_string(), tmp.path().to_path_buf());
    assert_eq!(
        MeNotifyProgram::new(&runtime).expect_err("program should fail").to_string(),
        MeNotifyError::MissingEntrypoint("broken".to_string()).to_string()
    );
}
