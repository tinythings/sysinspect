use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

/// Create a unique temporary runtime root under system temp directory
/// # Returns
/// * `PathBuf` - Temporary runtime root path
fn mk_tmp_runtime_root() -> PathBuf {
    let mut root = std::env::temp_dir();
    let pid = std::process::id();
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).map(|v| v.as_nanos()).unwrap_or_default();
    root.push(format!("sysinspect-py3-runtime-test-{pid}-{ns}"));
    root
}

/// Write a Python test module into runtime sharelib tree
/// # Arguments
/// * `root` - Temporary runtime root path
fn write_test_module(root: &std::path::Path) {
    let moddir = root.join("lib/runtime/python3");
    if let Err(err) = fs::create_dir_all(moddir.join("site-packages")) {
        panic!("failed to create test runtime tree: {err}");
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
    write_test_module(&root);

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.to_string_lossy() },
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

    let _ = fs::remove_dir_all(&root);
}
