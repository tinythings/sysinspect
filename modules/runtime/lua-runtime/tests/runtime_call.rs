use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use tempfile::TempDir;

fn mk_tmp_runtime_root() -> TempDir {
    tempfile::Builder::new()
        .prefix("sysinspect-lua-runtime-test-")
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary runtime root: {err}"))
}

fn write_test_module(root: &std::path::Path) {
    let moddir = root.join("lib/runtime/lua");
    let pkgdir = moddir.join("site-lua/mathx");
    if let Err(err) = fs::create_dir_all(&pkgdir) {
        panic!("failed to create test runtime tree: {err}");
    }

    if let Err(err) = fs::write(
        pkgdir.join("init.lua"),
        r#"
local M = {}

function M.double(v)
    return v * 2
end

return M
"#,
    ) {
        panic!("failed to write test lua package: {err}");
    }

    if let Err(err) = fs::write(
        pkgdir.join("helper.lua"),
        r#"
local M = {}

function M.triple(v)
    return v * 3
end

return M
"#,
    ) {
        panic!("failed to write helper lua package module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("hello.lua"),
        r#"
return {
    run = function(req)
        return { hello = req.args.name, value = 42, items = { "a", "b" } }
    end
}
"#,
    ) {
        panic!("failed to write test lua module: {err}");
    }

    if let Err(err) = fs::create_dir_all(moddir.join("__pycache__")) {
        panic!("failed to create ignored test directory: {err}");
    }

    if let Err(err) = fs::write(moddir.join("__pycache__/ignored.lua"), "return {}\n") {
        panic!("failed to write ignored lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("reader.lua"),
        r#"
return {
    doc = {
        name = "reader",
        description = "Reader module",
        arguments = {},
        options = {}
    },
    run = function(req)
        log.info("reader", req.args.name)
        return { reader = req.args.name }
    end
}
"#,
    ) {
        panic!("failed to write reader lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("importer.lua"),
        r#"
local mathx = require("mathx")

return {
    doc = {
        name = "importer",
        description = "Importer module",
        arguments = {},
        options = {}
    },
    run = function(req)
        return { doubled = mathx.double(req.args.value) }
    end
}
"#,
    ) {
        panic!("failed to write importer lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("badret.lua"),
        r#"
return {
    run = function(_req)
        return function() end
    end
}
"#,
    ) {
        panic!("failed to write bad return lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("echoreq.lua"),
        r#"
return {
    run = function(req)
        return {
            args = req.args,
            config_value = req.config["custom.flag"],
            opts = req.opts,
            ext = req.ext,
            host = req.host,
            host_name = req.host.traits["system.hostname"],
            host_sharelib = req.host.paths.sharelib,
            types = {
                truth = true,
                nothing = vim and nil or nil,
                items = { 1, "two", false, vim and nil or nil },
                nested = { value = 3.5 }
            }
        }
    end
}
"#,
    ) {
        panic!("failed to write echo request lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("hostecho.lua"),
        r#"
return {
    run = function(req)
        return {
            host = req.host,
            host_name = req.host.sys.hostname.short,
            sharelib = req.host.paths.sharelib
        }
    end
}
"#,
    ) {
        panic!("failed to write host echo lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("baddoc.lua"),
        r#"
return {
    doc = {
        description = "Missing required name field"
    },
    run = function(_req)
        return { ok = true }
    end
}
"#,
    ) {
        panic!("failed to write bad doc lua module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("pkgavail.lua"),
        r#"
return {
    run = function(_req)
        return {
            available = packagekit.available(),
            remove = type(packagekit.remove),
            upgrade = type(packagekit.upgrade)
        }
    end
}
"#,
    ) {
        panic!("failed to write packagekit helper test lua module: {err}");
    }
}

fn run_runtime(payload: &Value) -> Value {
    let bin = env!("CARGO_BIN_EXE_lua-runtime");
    let mut child = match Command::new(bin).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) => panic!("failed to spawn lua-runtime binary: {err}"),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if let Err(err) = stdin.write_all(payload.to_string().as_bytes()) {
            panic!("failed to write runtime request payload: {err}");
        }
    } else {
        panic!("lua-runtime stdin is not available");
    }

    let out = match child.wait_with_output() {
        Ok(out) => out,
        Err(err) => panic!("failed to wait for lua-runtime output: {err}"),
    };

    if !out.status.success() {
        panic!("lua-runtime exited with status {}", out.status);
    }

    match serde_json::from_slice::<Value>(&out.stdout) {
        Ok(val) => val,
        Err(err) => panic!("failed to parse lua-runtime JSON output: {err}"),
    }
}

#[test]
fn test_lua_runtime_returns_expected_json_payload() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "hello", "name": "Germany" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.get("message"), Some(&json!("Called Lua module successfully.")));
    assert_eq!(
        out.get("data"),
        Some(&json!({
            "changed": true,
            "data": {
                "hello": "Germany",
                "value": 42,
                "items": ["a", "b"]
            },
            "__sysinspect-module-logs": []
        }))
    );
}

#[test]
fn test_lua_runtime_returns_forwarded_logs() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["rt.logs"],
        "args": { "rt.mod": "reader", "name": "Germany" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/data"), Some(&json!({"reader": "Germany"})));
    let logs = out.pointer("/data/__sysinspect-module-logs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].as_str().unwrap_or_default().contains("[reader] reader Germany"));
}

#[test]
fn test_lua_runtime_passes_host_context_to_guest() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "host": {
            "sys": { "hostname": { "short": "lua-host" } },
            "paths": { "sharelib": "/srv/lua-share" }
        },
        "opts": [],
        "args": { "rt.mod": "hostecho" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/data/host_name"), Some(&json!("lua-host")));
    assert_eq!(out.pointer("/data/data/sharelib"), Some(&json!("/srv/lua-share")));
    assert_eq!(out.pointer("/data/data/host/sys/hostname/short"), Some(&json!("lua-host")));
}

#[test]
fn test_lua_runtime_exposes_packagekit_helper() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "pkgavail" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert!(out["data"]["data"]["available"].is_boolean());
    assert_eq!(out["data"]["data"]["remove"], json!("function"));
    assert_eq!(out["data"]["data"]["upgrade"], json!("function"));
}

#[test]
fn test_lua_runtime_lists_modules() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["rt.list"],
        "args": {}
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/modules"), Some(&json!(["baddoc", "badret", "echoreq", "hello", "hostecho", "importer", "pkgavail", "reader"])));
}

#[test]
fn test_lua_runtime_returns_module_doc() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "reader", "rt.man": true }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/name"), Some(&json!("reader")));
    assert_eq!(out.pointer("/data/description"), Some(&json!("Reader module")));
}

#[test]
fn test_lua_runtime_imports_from_site_lua_namespace() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "importer", "value": 21 }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/data"), Some(&json!({"doubled": 42})));
}

#[test]
fn test_lua_runtime_reports_missing_module() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "missing" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(1)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("Failed to execute Lua code"));
}

#[test]
fn test_lua_runtime_reports_non_json_return_value() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "badret" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(1)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("Lua run() must return table or JSON string"));
}

#[test]
fn test_lua_runtime_preserves_request_sections_and_json_types() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy(), "custom.flag": "seen" },
        "host": {
            "traits": { "system.hostname": "lua-minion" },
            "paths": { "sharelib": "/srv/lua-share" }
        },
        "opts": ["alpha", "beta", "rt.logs"],
        "args": { "rt.mod": "echoreq", "name": "Germany", "enabled": true, "count": 7 },
        "trace_id": "abc-123",
        "payload": { "items": [1, 2, 3] }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(
        out.pointer("/data/data"),
        Some(&json!({
            "args": { "name": "Germany", "enabled": true, "count": 7 },
            "config_value": "seen",
            "opts": ["alpha", "beta"],
            "ext": { "trace_id": "abc-123", "payload": { "items": [1, 2, 3] } },
            "host": {
                "traits": { "system.hostname": "lua-minion" },
                "paths": { "sharelib": "/srv/lua-share" }
            },
            "host_name": "lua-minion",
            "host_sharelib": "/srv/lua-share",
            "types": {
                "truth": true,
                "items": [1, "two", false],
                "nested": { "value": 3.5 }
            }
        }))
    );
}

#[test]
fn test_lua_runtime_rejects_invalid_module_doc() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "baddoc", "rt.man": true }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(1)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("doc.name is required"));
}
