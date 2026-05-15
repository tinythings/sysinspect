use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_ipfw") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("ipfw");
    assert!(p.exists(), "ipfw binary not found at {}", p.display());
    p
}

fn run_module(payload: &Value) -> Value {
    let bin = bin_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn {} binary: {}", bin.display(), err));

    child
        .stdin
        .as_mut()
        .expect("ipfw stdin not available")
        .write_all(payload.to_string().as_bytes())
        .expect("failed to write request payload");

    let out = child.wait_with_output().expect("failed to wait for ipfw output");
    let _ = out.status;
    serde_json::from_slice(&out.stdout).unwrap_or_else(|_| json!({"retcode": 1, "message": "binary exited with non-zero"}))
}

#[test]
fn dry_run_check_lists_rules() {
    let out = run_module(&json!({
        "options": ["check", "dry-run"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("[dry-run]"));
}

#[test]
fn dry_run_present_shows_rule() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": {"action": "allow", "port": "80"}
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("[dry-run]"));
    assert!(out["message"].as_str().unwrap().contains("80"));
}

#[test]
fn dry_run_present_block_ssh() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": {"action": "deny", "port": "22", "comment": "block SSH"}
    }));
    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("[dry-run]"));
    assert!(msg.contains("22"));
}

#[test]
fn dry_run_flush() {
    let out = run_module(&json!({
        "options": ["flush", "dry-run"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("[dry-run]"));
}

#[test]
fn missing_operation_returns_error() {
    let out = run_module(&json!({
        "options": [],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("No operation"));
}

#[test]
fn rule_with_source_subnet() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": {"action": "allow", "source": "10.0.0.0/8", "port": "5432"}
    }));
    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("10.0.0.0/8"));
    assert!(msg.contains("5432"));
}

#[test]
fn rule_block_icmp() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": {"action": "deny", "protocol": "icmp"}
    }));
    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("icmp"));
}

#[test]
fn rule_with_interface() {
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": {"action": "allow", "interface": "eth0", "port": "443"}
    }));
    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("443"));
}
