use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};
use tempfile::TempDir;

static HELLO_GO_MOD: &str = include_str!("../examples/hello/go.mod");
static HELLO_MAIN_GO: &str = include_str!("../examples/hello/main.go");
static CALLER_GO_MOD: &str = include_str!("../examples/caller/go.mod");
static CALLER_MAIN_GO: &str = include_str!("../examples/caller/main.go");
static CALLER_SYSINSPECT_GO: &str = include_str!("../examples/caller/sysinspect/sysinspect.go");

fn mk_tmp_runtime_root() -> TempDir {
    tempfile::Builder::new()
        .prefix("sysinspect-wasm-runtime-test-")
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary runtime root: {err}"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| panic!("failed to resolve repository root from {}", env!("CARGO_MANIFEST_DIR")))
}

fn wasm_cache_dir() -> &'static Path {
    static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
    CACHE_DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("sysinspect-wasm-runtime-test-cache-{}", std::process::id()));
        if let Err(err) = fs::create_dir_all(&dir) {
            panic!("failed to create wasm cache directory {}: {err}", dir.display());
        }
        dir
    })
}

fn build_go_example(example_dir: &Path, output_name: &str) -> PathBuf {
    let out = wasm_cache_dir().join(output_name);
    if out.exists() {
        return out;
    }

    let status = Command::new("go")
        .current_dir(example_dir)
        .env("GOOS", "wasip1")
        .env("GOARCH", "wasm")
        .arg("build")
        .arg("-trimpath")
        .arg("-ldflags=-s -w")
        .arg("-o")
        .arg(&out)
        .arg("main.go")
        .status()
        .unwrap_or_else(|err| panic!("failed to build Go wasm example in {}: {err}", example_dir.display()));
    if !status.success() {
        panic!("Go wasm build failed in {} with status {}", example_dir.display(), status);
    }

    out
}

fn stage_go_example(name: &str, files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("sysinspect-wasm-runtime-{name}-"))
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary Go example directory: {err}"));

    for (rel, body) in files {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("failed to create {}: {err}", parent.display()));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
    }

    dir
}

fn prebuilt_rs_reader() -> Option<PathBuf> {
    let path = repo_root().join("modules/runtime/wasm-runtime/examples/rs-reader/target/release/lib/runtime/wasm/rs-reader.wasm");
    path.exists().then_some(path)
}

fn install_module(root: &Path, src: &Path, dst_name: &str) {
    let moddir = root.join("lib/runtime/wasm");
    if let Err(err) = fs::create_dir_all(&moddir) {
        panic!("failed to create test runtime tree: {err}");
    }
    if let Err(err) = fs::copy(src, moddir.join(dst_name)) {
        panic!("failed to copy wasm module {}: {err}", src.display());
    }
}

fn install_test_modules(root: &Path) -> Vec<String> {
    let hello_src = stage_go_example("hello", &[("go.mod", HELLO_GO_MOD), ("main.go", HELLO_MAIN_GO)]);
    let hello = build_go_example(hello_src.path(), "hellodude.wasm");
    install_module(root, &hello, "hellodude.wasm");
    let caller_src =
        stage_go_example("caller", &[("go.mod", CALLER_GO_MOD), ("main.go", CALLER_MAIN_GO), ("sysinspect/sysinspect.go", CALLER_SYSINSPECT_GO)]);
    let caller = build_go_example(caller_src.path(), "caller.wasm");
    install_module(root, &caller, "caller.wasm");
    let mut modules = vec!["caller".to_string(), "hellodude".to_string()];
    if let Some(rs_reader) = prebuilt_rs_reader() {
        install_module(root, &rs_reader, "rs-reader.wasm");
        modules.push("rs-reader".to_string());
    }
    modules.sort();
    modules
}

fn run_runtime(payload: &Value) -> Value {
    let bin = env!("CARGO_BIN_EXE_wasm-runtime");
    let mut child = match Command::new(bin).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) => panic!("failed to spawn wasm-runtime binary: {err}"),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if let Err(err) = stdin.write_all(payload.to_string().as_bytes()) {
            panic!("failed to write runtime request payload: {err}");
        }
    } else {
        panic!("wasm-runtime stdin is not available");
    }

    let out = match child.wait_with_output() {
        Ok(out) => out,
        Err(err) => panic!("failed to wait for wasm-runtime output: {err}"),
    };

    if !out.status.success() {
        panic!("wasm-runtime exited with status {}", out.status);
    }

    match serde_json::from_slice::<Value>(&out.stdout) {
        Ok(val) => val,
        Err(err) => panic!("failed to parse wasm-runtime JSON output: {err}"),
    }
}

#[test]
fn test_wasm_runtime_returns_expected_json_payload() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "hellodude", "key": "VERSION" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.get("message"), Some(&json!("Wasm runtime executed successfully")));
    assert_eq!(out.pointer("/data/output"), Some(&json!("hello, dude")));
    assert!(out.pointer("/data/VERSION").and_then(|v| v.as_str()).is_some());
    assert_eq!(out.pointer("/data/__sysinspect-module-logs"), Some(&json!([])));
}

#[test]
fn test_wasm_runtime_honours_guest_options() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["nohello"],
        "args": { "rt.mod": "hellodude", "key": "VERSION" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert!(out.pointer("/data/VERSION").and_then(|v| v.as_str()).is_some());
    assert_eq!(out.pointer("/data/output"), None);
}

#[test]
fn test_wasm_runtime_returns_forwarded_logs() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "caller" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert!(out.pointer("/data/output").and_then(|v| v.as_str()).unwrap_or_default().contains("Linux"));
    let logs = out.pointer("/data/__sysinspect-module-logs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assert!(!logs.is_empty());
}

#[test]
fn test_wasm_runtime_lists_modules() {
    let root = mk_tmp_runtime_root();
    let expected_modules = install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": ["rt.list"],
        "args": {}
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/modules"), Some(&json!(expected_modules)));
}

#[test]
fn test_wasm_runtime_returns_module_doc_from_rt_man() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "hellodude", "rt.man": true }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/name"), Some(&json!("hellodude")));
    assert!(out.pointer("/data/description").and_then(|v| v.as_str()).is_some());
}

#[test]
fn test_wasm_runtime_reports_missing_module() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "missing" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(4)));
    assert!(out.get("message").and_then(|v| v.as_str()).unwrap_or_default().contains("Module \"missing\" was not found"));
}

#[test]
fn test_wasm_runtime_runs_rust_guest_module() {
    let root = mk_tmp_runtime_root();
    let modules = install_test_modules(root.path());
    if !modules.iter().any(|m| m == "rs-reader") {
        return;
    }

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "rs-reader" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    let minion_id = out.pointer("/data/minion_id").and_then(|v| v.as_str()).unwrap_or_default();
    let err = out.pointer("/data/error").and_then(|v| v.as_str()).unwrap_or_default();
    assert!(!minion_id.is_empty() || err == "Could not read machine-id file");
}
