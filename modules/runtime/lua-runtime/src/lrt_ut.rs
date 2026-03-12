use crate::lrt::LuaRuntime;
use serde_json::json;
use tempfile::TempDir;

/// Create a temporary sharelib root for Lua runtime unit tests.
///
/// Returns:
/// * `TempDir` rooted temporary sharelib tree.
fn mk_tmp_runtime_root() -> TempDir {
    tempfile::Builder::new()
        .prefix("sysinspect-lua-runtime-ut-")
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary runtime root: {err}"))
}

#[test]
fn call_module_drains_forwarded_logs_between_calls() {
    let root = mk_tmp_runtime_root();
    let rt = LuaRuntime::new(root.path().to_path_buf(), false).unwrap_or_else(|err| panic!("failed to create lua runtime: {err}"));
    let code = r#"
return {
    run = function(_req)
        log.info("hello from lua")
        return { ok = true }
    end
}
"#;

    let first = rt
        .call_module("drainlogs", code, &json!({"args": {}, "config": {}, "opts": [], "ext": {}}), true)
        .unwrap_or_else(|err| panic!("first lua call should succeed: {err}"));
    let second = rt
        .call_module("drainlogs", code, &json!({"args": {}, "config": {}, "opts": [], "ext": {}}), false)
        .unwrap_or_else(|err| panic!("second lua call should succeed: {err}"));

    assert_eq!(first["__sysinspect-module-logs"].as_array().map(|v| v.len()), Some(1));
    assert_eq!(second["__sysinspect-module-logs"], json!([]));
}
