use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use tempfile::TempDir;

/// Create a temporary runtime root under system temp directory
/// # Returns
/// * `TempDir` - Temporary runtime root handle
fn mk_tmp_runtime_root() -> TempDir {
    tempfile::Builder::new()
        .prefix("sysinspect-py3-runtime-test-")
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary runtime root: {err}"))
}

/// Write a Python test module into runtime sharelib tree
/// # Arguments
/// * `root` - Temporary runtime root path
fn write_test_module(root: &std::path::Path) {
    let moddir = root.join("lib/runtime/python3");
    let pkgdir = moddir.join("site-packages/mathx");
    if let Err(err) = fs::create_dir_all(&pkgdir) {
        panic!("failed to create test runtime tree: {err}");
    }

    if let Err(err) = fs::write(
        pkgdir.join("__init__.py"),
        r#"
def double(v):
    return v * 2
"#,
    ) {
        panic!("failed to write test python package: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("hello.py"),
        r#"
def run(req):
    return {"hello": req["args"]["name"], "value": 42, "items": ["a", "b"]}
"#,
    ) {
        panic!("failed to write test python module: {err}");
    }

    if let Err(err) = fs::create_dir_all(moddir.join("nested")) {
        panic!("failed to create nested test python module directory: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("nested/reader.py"),
        r#"
def run(req):
    log.info("nested", req["args"]["name"])
    return {"nested": req["args"]["name"]}

def doc():
    return {
        "name": "nested.reader",
        "description": "Nested reader module",
        "arguments": [],
        "options": []
    }
"#,
    ) {
        panic!("failed to write nested test python module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("importer.py"),
        r#"
from mathx import double

doc = {
    "name": "importer",
    "description": "Importer module",
    "arguments": [],
    "options": []
}

def run(req):
    return {"doubled": double(req["args"]["value"])}
"#,
    ) {
        panic!("failed to write importer test python module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("badret.py"),
        r#"
def run(req):
    return set([1, 2, 3])
"#,
    ) {
        panic!("failed to write bad return test python module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("echoreq.py"),
        r#"
def run(req):
    return {
        "args": req["args"],
        "config_value": req["config"].get("custom.flag"),
        "opts": req["opts"],
        "ext": req["ext"],
        "types": {
            "truth": True,
            "nothing": None,
            "items": [1, "two", False, None],
            "nested": {"value": 3.5},
        },
    }
"#,
    ) {
        panic!("failed to write echo request test python module: {err}");
    }

    if let Err(err) = fs::write(
        moddir.join("baddoc.py"),
        r#"
doc = {
    "description": "Missing required name field"
}

def run(req):
    return {"ok": True}
"#,
    ) {
        panic!("failed to write bad doc test python module: {err}");
    }
}

/// Run `py3-runtime` binary with JSON request payload
/// # Arguments
/// * `payload` - Runtime request payload
/// # Returns
/// * `Value` - Parsed JSON response
fn run_runtime(payload: &Value) -> Value {
    let bin = env!("CARGO_BIN_EXE_py3-runtime");
    let mut child = match Command::new(bin).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) => panic!("failed to spawn py3-runtime binary: {err}"),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if let Err(err) = stdin.write_all(payload.to_string().as_bytes()) {
            panic!("failed to write runtime request payload: {err}");
        }
    } else {
        panic!("py3-runtime stdin is not available");
    }

    let out = match child.wait_with_output() {
        Ok(out) => out,
        Err(err) => panic!("failed to wait for py3-runtime output: {err}"),
    };

    if !out.status.success() {
        panic!("py3-runtime exited with status {}", out.status);
    }

    match serde_json::from_slice::<Value>(&out.stdout) {
        Ok(val) => val,
        Err(err) => panic!("failed to parse py3-runtime JSON output: {err}"),
    }
}

#[test]
fn test_python_runtime_returns_expected_json_payload() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "hello", "name": "Germany" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.get("message"), Some(&json!("Called Python module successfully.")));
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
fn test_python_runtime_returns_forwarded_logs() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["rt.logs"],
        "args": { "rt.mod": "nested.reader", "name": "Germany" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/data"), Some(&json!({"nested": "Germany"})));
    let logs = out.pointer("/data/__sysinspect-module-logs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].as_str().unwrap_or_default().contains("[nested.reader] nested Germany"));
}

#[test]
fn test_python_runtime_lists_nested_modules() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["rt.list"],
        "args": {}
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(
        out.pointer("/data/modules"),
        Some(&json!(["baddoc", "badret", "echoreq", "hello", "importer", "nested.reader"]))
    );
}

#[test]
fn test_python_runtime_returns_module_doc_from_doc_function() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "nested.reader", "rt.man": true }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/name"), Some(&json!("nested.reader")));
    assert_eq!(out.pointer("/data/description"), Some(&json!("Nested reader module")));
}

#[test]
fn test_python_runtime_imports_from_site_packages_namespace() {
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
fn test_python_runtime_returns_module_doc_from_doc_object() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "importer", "rt.man": true }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/name"), Some(&json!("importer")));
    assert_eq!(out.pointer("/data/description"), Some(&json!("Importer module")));
}

#[test]
fn test_python_runtime_reports_missing_module() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "missing.module" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(1)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("Failed to read Python module"));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("missing/module.py"));
}

#[test]
fn test_python_runtime_reports_non_json_return_value() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "badret" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(1)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("Unable to serialise Python value to JSON"));
}

#[test]
fn test_python_runtime_preserves_request_sections_and_json_types() {
    let root = mk_tmp_runtime_root();
    write_test_module(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy(), "custom.flag": "seen" },
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
            "types": {
                "truth": true,
                "nothing": null,
                "items": [1, "two", false, null],
                "nested": { "value": 3.5 }
            }
        }))
    );
}

#[test]
fn test_python_runtime_rejects_invalid_module_doc() {
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
