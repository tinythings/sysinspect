use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_pkg") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop(); // deps/
    p.pop(); // debug/ or release/
    p.push("pkg");
    assert!(p.exists(), "pkg binary not found at {}", p.display());
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
        .expect("pkg stdin is not available")
        .write_all(payload.to_string().as_bytes())
        .expect("failed to write module request payload");

    let out = child.wait_with_output().expect("failed to wait for pkg output");
    assert!(out.status.success(), "pkg exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).expect("failed to parse pkg JSON output")
}

#[test]
fn dry_run_install_shows_command() {
    let out = run_module(&json!({
        "options": ["install", "dry-run"],
        "arguments": { "name": "testpkg" }
    }));

    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.starts_with("[dry-run] "));
    assert!(msg.contains("testpkg"));
}

#[test]
fn dry_run_search_shows_command() {
    let out = run_module(&json!({
        "options": ["search", "dry-run"],
        "arguments": { "name": "nginx" }
    }));

    assert_eq!(out["retcode"], 0);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.starts_with("[dry-run] "));
    assert!(msg.contains("nginx"));
}

#[test]
fn dry_run_update_shows_command() {
    let out = run_module(&json!({
        "options": ["update", "dry-run"],
        "arguments": {}
    }));

    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().starts_with("[dry-run] "));
}

#[test]
fn dry_run_upgrade_shows_command() {
    let out = run_module(&json!({
        "options": ["upgrade", "dry-run"],
        "arguments": {}
    }));

    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().starts_with("[dry-run] "));
}

#[test]
fn no_operation_returns_error() {
    let out = run_module(&json!({
        "options": [],
        "arguments": {}
    }));

    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("No operation specified"));
}

#[test]
fn remove_requires_name() {
    let out = run_module(&json!({
        "options": ["remove", "dry-run"],
        "arguments": { "name": "testpkg" }
    }));

    assert_eq!(out["retcode"], 0);
    assert!(out["message"].as_str().unwrap().contains("testpkg"));
}
