use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_dir") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("dir");
    assert!(p.exists(), "dir binary not found at {}", p.display());
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
        .expect("dir stdin is not available")
        .write_all(payload.to_string().as_bytes())
        .expect("failed to write module request payload");

    let out = child.wait_with_output().expect("failed to wait for dir output");
    assert!(out.status.success(), "dir exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).expect("failed to parse dir JSON output")
}

fn tmpdir() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CNT: AtomicU32 = AtomicU32::new(0);
    let n = CNT.fetch_add(1, Ordering::Relaxed);
    let d = format!("/tmp/dir-test-{}-{}", std::process::id(), n);
    let _ = fs::remove_dir_all(&d);
    d
}

// -------- check --------

#[test]
fn check_existing_directory() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": "/tmp" }
    }));
    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["name"], "/tmp");
    assert_eq!(out["data"]["exists"], true);
    assert_eq!(out["data"]["is_dir"], true);
}

#[test]
fn check_nonexistent_directory() {
    let d = tmpdir();
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["name"], d);
    assert_eq!(out["data"]["exists"], false);
}

#[test]
fn check_missing_name_returns_error() {
    let out = run_module(&json!({
        "options": ["check"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("name"));
}

// -------- present --------

#[test]
fn present_creates_directory() {
    let d = tmpdir();
    let out = run_module(&json!({
        "options": ["present"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(fs::metadata(&d).unwrap().is_dir());
    fs::remove_dir(&d).unwrap();
}

#[test]
fn present_idempotent_existing_directory() {
    let d = tmpdir();
    fs::create_dir(&d).unwrap();
    let actual_mode = format!("{:04o}", fs::metadata(&d).unwrap().permissions().mode() & 0o777);
    let out = run_module(&json!({
        "options": ["present"],
        "arguments": { "name": d.as_str(), "mode": actual_mode }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("already exists"));
    fs::remove_dir(&d).unwrap();
}

#[test]
fn present_with_mode() {
    let d = tmpdir();
    let out = run_module(&json!({
        "options": ["present"],
        "arguments": { "name": d, "mode": "0700" }
    }));
    assert_eq!(out["retcode"], 0);
    let meta = fs::metadata(&d).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    fs::remove_dir(&d).unwrap();
}

#[test]
fn present_missing_name_returns_error() {
    let out = run_module(&json!({
        "options": ["present"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("name"));
}

// -------- absent --------

#[test]
fn absent_removes_directory() {
    let d = tmpdir();
    fs::create_dir(&d).unwrap();
    let out = run_module(&json!({
        "options": ["absent"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(!fs::metadata(&d).is_ok());
}

#[test]
fn absent_idempotent_nonexistent() {
    let d = tmpdir();
    let out = run_module(&json!({
        "options": ["absent"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("does not exist"));
}

#[test]
fn absent_missing_name_returns_error() {
    let out = run_module(&json!({
        "options": ["absent"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("name"));
}

// -------- dry-run --------

#[test]
fn dry_run_present_shows_would_create() {
    let d = tmpdir();
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("[dry-run]"));
    assert!(out["message"].as_str().unwrap().contains("would create"));
    assert!(!fs::metadata(&d).is_ok());
}

#[test]
fn dry_run_present_shows_already_exists() {
    let d = tmpdir();
    fs::create_dir(&d).unwrap();
    let out = run_module(&json!({
        "options": ["present", "dry-run"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("already exists"));
    fs::remove_dir(&d).unwrap();
}

#[test]
fn dry_run_absent_shows_would_remove() {
    let d = tmpdir();
    fs::create_dir(&d).unwrap();
    let out = run_module(&json!({
        "options": ["absent", "dry-run"],
        "arguments": { "name": d }
    }));
    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("would remove"));
    assert!(fs::metadata(&d).unwrap().is_dir());
    fs::remove_dir(&d).unwrap();
}

// -------- validation --------

#[test]
fn no_operation_returns_error() {
    let out = run_module(&json!({
        "options": [],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("No operation"));
}
