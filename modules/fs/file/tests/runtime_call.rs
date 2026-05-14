use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU32, Ordering},
};

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn tmp_path(prefix: &str) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("fsfile-it-{}-{}-{}", std::process::id(), prefix, n))
}

fn bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_file") {
        return PathBuf::from(p);
    }
    let mut p = std::env::current_exe().expect("cannot locate test executable");
    p.pop();
    p.pop();
    p.push("file");
    assert!(p.exists(), "file binary not found at {}", p.display());
    p
}

fn run_module(payload: &Value) -> Value {
    let bin = bin_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn {} binary: {}", bin.display(), err));

    child.stdin.as_mut().unwrap().write_all(payload.to_string().as_bytes()).unwrap();

    let out = child.wait_with_output().expect("failed to wait for file output");
    assert!(out.status.success(), "file exited with status {}", out.status);

    serde_json::from_slice(&out.stdout).expect("failed to parse file JSON output")
}

// ---------------------------------------------------------------------------
// line-present
// ---------------------------------------------------------------------------

#[test]
fn line_present_adds_line_when_missing() {
    let path = tmp_path("lp-add");
    let initial = "existing line\n";
    fs::write(&path, initial).unwrap();

    let out = run_module(&json!({
        "options": ["line-present"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "new line"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("existing line"));
    assert!(contents.contains("new line"));
    fs::remove_file(&path).ok();
}

#[test]
fn line_present_skips_when_line_exists() {
    let path = tmp_path("lp-skip");
    fs::write(&path, "foo=bar\nbaz=qux\n").unwrap();

    let out = run_module(&json!({
        "options": ["line-present"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "foo=bar"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], false);
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("already present"));
    fs::remove_file(&path).ok();
}

#[test]
fn line_present_idempotent() {
    let path = tmp_path("lp-idem");
    fs::write(&path, "original\n").unwrap();

    let payload = json!({
        "options": ["line-present"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "guard-line"
        }
    });

    let out1 = run_module(&payload);
    assert_eq!(out1["data"]["changed"], true);

    let out2 = run_module(&payload);
    assert_eq!(out2["retcode"], 0);
    assert_eq!(out2["data"]["changed"], false);
    fs::remove_file(&path).ok();
}

#[test]
fn line_present_creates_missing_file_in_easy_mode() {
    let path = tmp_path("lp-create");

    let out = run_module(&json!({
        "options": ["line-present"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "only-line"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    assert!(path.exists());
    let contents = fs::read_to_string(&path).unwrap();
    assert_eq!(contents.trim(), "only-line");
    fs::remove_file(&path).ok();
}

#[test]
fn line_present_strict_mode_errors_on_missing_file() {
    let path = tmp_path("lp-strict");
    // ensure file does not exist
    fs::remove_file(&path).ok();

    let out = run_module(&json!({
        "options": ["line-present"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "some-line",
            "mode": "strict"
        }
    }));

    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("does not exist"));
}

// ---------------------------------------------------------------------------
// line-absent
// ---------------------------------------------------------------------------

#[test]
fn line_absent_removes_line_when_present() {
    let path = tmp_path("la-remove");
    fs::write(&path, "to-keep\nto-remove\nanother-keep\n").unwrap();

    let out = run_module(&json!({
        "options": ["line-absent"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "to-remove"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    let contents = fs::read_to_string(&path).unwrap();
    assert!(!contents.contains("to-remove"));
    assert!(contents.contains("to-keep"));
    assert!(contents.contains("another-keep"));
    fs::remove_file(&path).ok();
}

#[test]
fn line_absent_skips_when_line_missing() {
    let path = tmp_path("la-skip");
    fs::write(&path, "something\nelse\n").unwrap();

    let out = run_module(&json!({
        "options": ["line-absent"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "not-here"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], false);
    fs::remove_file(&path).ok();
}

#[test]
fn line_absent_idempotent() {
    let path = tmp_path("la-idem");
    fs::write(&path, "a\nb\nc\n").unwrap();

    let payload = json!({
        "options": ["line-absent"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "b"
        }
    });

    let out1 = run_module(&payload);
    assert_eq!(out1["data"]["changed"], true);
    assert!(!fs::read_to_string(&path).unwrap().contains("b"));

    let out2 = run_module(&payload);
    assert_eq!(out2["data"]["changed"], false);
    fs::remove_file(&path).ok();
}

#[test]
fn line_absent_removes_all_duplicates() {
    let path = tmp_path("la-dupes");
    fs::write(&path, "dup\nunique\ndup\n").unwrap();

    let out = run_module(&json!({
        "options": ["line-absent"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "dup"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    let contents = fs::read_to_string(&path).unwrap();
    assert!(!contents.contains("dup"));
    assert!(contents.contains("unique"));
    fs::remove_file(&path).ok();
}

#[test]
fn line_absent_nonexistent_file_is_ok() {
    let path = tmp_path("la-nofile");
    fs::remove_file(&path).ok();

    let out = run_module(&json!({
        "options": ["line-absent"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "anything"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], false);
    assert!(out["message"].as_str().unwrap().contains("does not exist"));
}

// ---------------------------------------------------------------------------
// replace
// ---------------------------------------------------------------------------

#[test]
fn replace_substitutes_text() {
    let path = tmp_path("rp-sub");
    fs::write(&path, "listen 80\nlisten 443\n").unwrap();

    let out = run_module(&json!({
        "options": ["replace"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "80",
            "value": "8080"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("listen 8080"));
    assert!(!contents.lines().any(|l| l == "listen 80"));
    fs::remove_file(&path).ok();
}

#[test]
fn replace_skips_when_no_match() {
    let path = tmp_path("rp-skip");
    fs::write(&path, "original\ncontent\n").unwrap();

    let out = run_module(&json!({
        "options": ["replace"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "not-here",
            "value": "replacement"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], false);
    assert!(out["message"].as_str().unwrap().contains("No matches"));
    let contents = fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "original\ncontent\n");
    fs::remove_file(&path).ok();
}

#[test]
fn replace_multiple_occurrences() {
    let path = tmp_path("rp-multi");
    fs::write(&path, "foo bar foo\nfoo baz\n").unwrap();

    let out = run_module(&json!({
        "options": ["replace"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "foo",
            "value": "qux"
        }
    }));

    assert_eq!(out["retcode"], 0);
    assert_eq!(out["data"]["changed"], true);
    let contents = fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "qux bar qux\nqux baz\n");
    fs::remove_file(&path).ok();
}

#[test]
fn replace_idempotent() {
    let path = tmp_path("rp-idem");
    fs::write(&path, "old-value\n").unwrap();

    let payload = json!({
        "options": ["replace"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "old",
            "value": "new"
        }
    });

    let out1 = run_module(&payload);
    assert_eq!(out1["data"]["changed"], true);
    assert_eq!(fs::read_to_string(&path).unwrap(), "new-value\n");

    let out2 = run_module(&payload);
    assert_eq!(out2["data"]["changed"], false);
    fs::remove_file(&path).ok();
}

#[test]
fn replace_strict_mode_errors_on_missing_file() {
    let path = tmp_path("rp-strict");
    fs::remove_file(&path).ok();

    let out = run_module(&json!({
        "options": ["replace"],
        "arguments": {
            "name": path.to_str().unwrap(),
            "pattern": "foo",
            "value": "bar",
            "mode": "strict"
        }
    }));

    assert_eq!(out["retcode"], 1);
    assert!(out["message"].as_str().unwrap().contains("does not exist"));
}
