use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_facts") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("facts");
    assert!(p.exists(), "facts binary not found at {}", p.display());
    p
}

fn run(payload: &str) -> serde_json::Value {
    let bin = bin_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn {}: {}", bin.display(), err));
    child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).unwrap();
    let out = child.wait_with_output().expect("failed to wait for facts output");
    serde_json::from_slice(&out.stdout).expect("failed to parse facts JSON output")
}

#[test]
fn gather_returns_all_keys() {
    let out = run(r#"{"options":["gather"]}"#);
    assert_eq!(out["retcode"], 0);
    let d = &out["data"];
    assert_eq!(d["os"], "linux");
    assert!(!d["hostname"].as_str().unwrap().is_empty());
    assert!(d["memory_total_kb"].as_str().is_some());
    assert!(d["cpu_model"].as_str().is_some());
    assert!(d["uptime_seconds"].as_str().is_some());
}

#[test]
fn list_keys_returns_array() {
    let out = run(r#"{"options":["list-keys"]}"#);
    assert_eq!(out["retcode"], 0);
    let arr = out["data"].as_array().unwrap();
    assert!(arr.iter().any(|v| v.as_str() == Some("os")));
    assert!(arr.iter().any(|v| v.as_str() == Some("hostname")));
}

#[test]
fn get_returns_single_key() {
    let out = run(r#"{"options":["get"],"arguments":{"key":"os"}}"#);
    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["os"], "linux");
}

#[test]
fn no_operation_is_error() {
    let out = run(r#"{"options":[]}"#);
    assert_eq!(out["retcode"], 1);
}

#[test]
fn unknown_key_returns_empty() {
    let out = run(r#"{"options":["get"],"arguments":{"key":"nonexistent"}}"#);
    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"].as_object().unwrap().len(), 0);
}
