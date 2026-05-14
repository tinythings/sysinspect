use serde_json::{Value, json};
use std::{
    io::Write,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Stdio},
};

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_net") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("net");
    assert!(p.exists(), "net binary not found at {}", p.display());
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
    let out = child.wait_with_output().expect("failed to wait for net output");
    assert!(out.status.success(), "net exited with status {}", out.status);
    serde_json::from_slice(&out.stdout).expect("failed to parse net JSON output")
}

#[test]
fn connect_open_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port().to_string();
    drop(listener);
    // Port is now free, but we test that a freshly-bound port was reachable when it existed.
    // Instead, test with a listener still open:
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let out = run_module(&json!({
        "options": ["connect"],
        "arguments": { "host": "127.0.0.1", "port": addr.port().to_string() }
    }));

    assert!(out["data"]["open"].as_bool().unwrap());
    assert_eq!(out["data"]["host"], "127.0.0.1");
    assert!(out["data"]["latency_ms"].as_u64().is_some());
}

#[test]
fn connect_closed_port() {
    let out = run_module(&json!({
        "options": ["connect"],
        "arguments": { "host": "127.0.0.1", "port": "19999" }
    }));

    assert!(!out["data"]["open"].as_bool().unwrap());
}

#[test]
fn connect_missing_args() {
    let out = run_module(&json!({
        "options": ["connect"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
}

#[test]
fn ping_localhost() {
    let out = run_module(&json!({
        "options": ["ping"],
        "arguments": { "host": "127.0.0.1", "count": "1", "timeout": "2" }
    }));

    let data = &out["data"];
    assert_eq!(data["host"], "127.0.0.1");
    assert!(data["sent"].as_u64().is_some());
    assert!(data["received"].as_u64().is_some());
    assert!(data["loss_pct"].as_f64().is_some());
}

#[test]
fn ping_missing_host() {
    let out = run_module(&json!({
        "options": ["ping"],
        "arguments": {}
    }));
    assert_eq!(out["retcode"], 1);
}
