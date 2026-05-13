use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_service") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("service");
    assert!(p.exists(), "service binary not found at {}", p.display());
    p
}

fn run_module(payload: &Value) -> Value {
    let bin = bin_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn {}: {}", bin.display(), err));

    child.stdin.as_mut().unwrap().write_all(payload.to_string().as_bytes()).unwrap();

    let out = child.wait_with_output().expect("failed to wait for service output");
    assert!(out.status.success(), "service exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).expect("failed to parse service JSON output")
}

#[test]
fn check_sshd_returns_telemetry() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": "sshd" }
    }));

    assert_eq!(out["retcode"], 0);
    let data = &out["data"];
    assert_eq!(data["name"], "sshd");
    assert!(data.get("manager").and_then(|v| v.as_str()).is_some());
}

#[test]
fn status_returns_running_or_not() {
    let out = run_module(&json!({
        "options": ["status"],
        "arguments": { "name": "sshd" }
    }));

    let retcode = out["retcode"].as_i64().unwrap();
    assert!(retcode == 0 || retcode == 1);
    assert!(out["data"].get("running").and_then(|v| v.as_bool()).is_some());
}

#[test]
fn dry_run_shows_command() {
    let out = run_module(&json!({
        "options": ["start", "dry-run"],
        "arguments": { "name": "test-service" }
    }));

    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.starts_with("[dry-run] "));
    assert!(msg.contains("test-service"));
}

#[test]
fn missing_name_returns_error() {
    let out = run_module(&json!({
        "options": ["status"],
        "arguments": {}
    }));

    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("name"));
}

#[test]
fn no_operation_returns_error() {
    let out = run_module(&json!({
        "options": [],
        "arguments": { "name": "sshd" }
    }));

    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("No operation"));
}

#[test]
fn dry_run_check_shows_status_command() {
    let out = run_module(&json!({
        "options": ["check", "dry-run"],
        "arguments": { "name": "test-service" }
    }));

    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().starts_with("[dry-run] "));
}

#[test]
fn info_returns_rich_telemetry() {
    let out = run_module(&json!({
        "options": ["info"],
        "arguments": { "name": "sshd" }
    }));

    assert_eq!(out["retcode"], 0);
    let data = &out["data"];
    assert_eq!(data["name"], "sshd");
    assert!(data.get("running").and_then(|v| v.as_bool()).is_some());
    // systemd hosts will have these extra fields
    if data.get("active_state").is_some() {
        assert!(data.get("load_state").is_some());
        assert!(data.get("sub_state").is_some());
    }
}

#[test]
fn check_nonexistent_service_returns_telemetry() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": "nonexistent-service-zzz" }
    }));

    assert_eq!(out["retcode"], 0);
    let data = &out["data"];
    assert_eq!(data["name"], "nonexistent-service-zzz");
    assert!(data.get("manager").is_some());
}
