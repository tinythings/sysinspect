use crate::{MeNotifyContext, MeNotifyProgram, MeNotifyRunner, MeNotifyRuntime};
use std::{fs, time::Duration};

#[test]
fn tick_runner_reuses_program_vm() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
seen = seen or 0
return {
  tick = function(_ctx)
    seen = seen + 1
  end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new("demo", "menotify.demo", "demo", &[], &serde_yaml::Value::Null, Some(Duration::from_secs(1))),
    );

    runner.run_tick().expect("first tick should succeed");
    runner.run_tick().expect("second tick should succeed");

    assert_eq!(
        runner
            .program()
            .lua()
            .globals()
            .get::<i64>("seen")
            .expect("global counter should exist"),
        2
    );
}

#[test]
fn loop_runner_calls_loop_entrypoint() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  loop = function(ctx)
    executed = ctx.module
  end
}
"#,
    )
    .expect("script file should be written");

    let runtime = MeNotifyRuntime::with_sharelib_root("demo".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime).expect("program should load"),
        MeNotifyContext::new("demo", "menotify.demo", "demo", &[], &serde_yaml::Value::Null, None),
    );

    runner.run_loop().expect("loop should succeed");
    assert_eq!(
        runner
            .program()
            .lua()
            .globals()
            .get::<String>("executed")
            .expect("execution marker should exist"),
        "demo"
    );
}
