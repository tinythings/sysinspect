use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_kernel") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("kernel");
    assert!(p.exists(), "kernel binary not found at {}", p.display());
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

    let out = child.wait_with_output().expect("failed to wait for kernel output");
    assert!(out.status.success(), "kernel exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).expect("failed to parse kernel JSON output")
}

fn has_manager() -> bool {
    let out = run_module(&json!({ "options": ["list"] }));
    !out["message"].as_str().unwrap_or("").contains("No kernel module manager")
}

#[test]
fn list_returns_modules() {
    if !has_manager() {
        return;
    }
    let out = run_module(&json!({ "options": ["list"] }));
    assert_eq!(out["retcode"], 0);
    let data = &out["data"];
    assert!(data.get("modules").and_then(|v| v.as_array()).is_some());
    assert!(data.get("manager").is_some());
}

#[test]
fn status_checks_module() {
    if !has_manager() {
        return;
    }
    let out = run_module(&json!({
        "options": ["status"],
        "arguments": { "name": "nonexistent_zzz_xyz" }
    }));
    let retcode = out["retcode"].as_i64().unwrap();
    assert!(retcode == 0 || retcode == 1);
    let data = &out["data"];
    assert!(data.get("loaded").and_then(|v| v.as_bool()).is_some());
}

#[test]
fn dry_run_shows_command() {
    let out = run_module(&json!({
        "options": ["load", "dry-run"],
        "arguments": { "name": "dummy-module" }
    }));
    let retcode = out["retcode"].as_i64().unwrap();
    assert!(retcode == 0 || out["message"].as_str().unwrap().contains("no kernel module manager"));
    if retcode == 0 {
        assert!(out["message"].as_str().unwrap().starts_with("[dry-run] "));
        assert!(out["message"].as_str().unwrap().contains("dummy-module"));
    }
}

#[test]
fn missing_name_returns_error() {
    if !has_manager() {
        return;
    }
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
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("No operation"));
}
