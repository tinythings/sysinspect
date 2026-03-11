use crate::{MeNotifyContext, MeNotifyEventBuilder, MeNotifyProgram, MeNotifyRunner, MeNotifyRuntime};
use std::{fs, sync::Mutex, time::Duration};

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

    assert_eq!(runner.program().lua().globals().get::<i64>("seen").expect("global counter should exist"), 2);
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
    assert_eq!(runner.program().lua().globals().get::<String>("executed").expect("execution marker should exist"), "demo");
}

#[test]
fn tick_runner_emits_sensor_event() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  tick = function(ctx)
    ctx.emit({ value = 42 }, { action = "created", key = "OPS-1" })
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
    let out = Mutex::new(Vec::new());

    runner
        .run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &MeNotifyEventBuilder::new("demo", "menotify.demo", None))
        .expect("tick with emit should succeed");

    let events = out.lock().expect("lock should work");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["eid"], "demo|menotify.demo|created@OPS-1|0");
    assert_eq!(events[0]["data"]["value"], 42);
}

#[test]
fn tick_runner_exposes_sleep_now_timestamp_and_log() {
    let tmp = tempfile::tempdir().expect("tempdir should be created");
    let root = tmp.path().join("lib/sensors/lua54");
    fs::create_dir_all(&root).expect("script root should be created");
    fs::write(
        root.join("demo.lua"),
        r#"
return {
  tick = function(ctx)
    log.info("hello", 123)
    ctx.sleep(0)
    ctx.emit({ now = ctx.now(), ts = ctx.timestamp() })
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
    let out = Mutex::new(Vec::new());

    runner
        .run_tick_with_emit(&|ev| out.lock().expect("lock should work").push(ev), &MeNotifyEventBuilder::new("demo", "menotify.demo", None))
        .expect("tick with host api should succeed");

    let events = out.lock().expect("lock should work");
    assert_eq!(events.len(), 1);
    assert!(events[0]["data"]["now"].as_f64().expect("now should be number") > 0.0);
    assert!(events[0]["data"]["ts"].as_str().expect("timestamp should be string").contains('T'));
}

#[test]
fn separate_runners_keep_isolated_vms() {
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

    let runtime_a = MeNotifyRuntime::with_sharelib_root("demo-a".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runtime_b = MeNotifyRuntime::with_sharelib_root("demo-b".to_string(), "menotify.demo".to_string(), tmp.path().to_path_buf());
    let runner_a = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime_a).expect("program a should load"),
        MeNotifyContext::new("demo-a", "menotify.demo", "demo", &[], &serde_yaml::Value::Null, Some(Duration::from_secs(1))),
    );
    let runner_b = MeNotifyRunner::new(
        MeNotifyProgram::new(&runtime_b).expect("program b should load"),
        MeNotifyContext::new("demo-b", "menotify.demo", "demo", &[], &serde_yaml::Value::Null, Some(Duration::from_secs(1))),
    );

    runner_a.run_tick().expect("runner a first tick should succeed");
    runner_a.run_tick().expect("runner a second tick should succeed");
    runner_b.run_tick().expect("runner b first tick should succeed");

    assert_eq!(runner_a.program().lua().globals().get::<i64>("seen").expect("runner a counter should exist"), 2);
    assert_eq!(runner_b.program().lua().globals().get::<i64>("seen").expect("runner b counter should exist"), 1);
}
