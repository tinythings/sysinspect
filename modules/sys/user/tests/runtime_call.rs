use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_user") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("user");
    assert!(p.exists(), "user binary not found at {}", p.display());
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
    let out = child.wait_with_output().expect("failed to wait for user output");
    assert!(out.status.success(), "user exited with status {}", out.status);
    serde_json::from_slice(&out.stdout).expect("failed to parse user JSON output")
}

#[test]
fn check_root_user() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": "root" }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["exists"], true);
    assert_eq!(out["data"]["uid"], 0);
    assert_eq!(out["data"]["name"], "root");
    assert!(!out["data"]["home"].as_str().unwrap().is_empty());
}

#[test]
fn check_nonexistent_user() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": "zzz_nonexistent_user_xyz" }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["exists"], false);
}

#[test]
fn missing_name_is_error() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("name"));
}

#[test]
fn no_operation_is_error() {
    let out = run_module(&json!({
        "options": [],
        "arguments": { "name": "root" }
    }));
    assert_eq!(out["retcode"], 1);
}

#[test]
fn present_dry_run() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": { "name": "testuser_dryrun", "uid": "9999", "home": "/tmp/testuser_dryrun" }
    }));

    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().starts_with("[dry-run]"));
}
