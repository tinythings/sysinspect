use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};
use tempfile::TempDir;

static HELLO_CARGO_TOML: &str = r#"
[package]
name = "hellodude"
version = "0.1.0"
edition = "2024"

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

static HELLO_MAIN_RS: &str = r##"
use std::io::{self, Read};

fn has_opt(src: &str, want: &str) -> bool {
    src.contains(&format!("\"{want}\""))
}

fn read_header() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).map_err(|err| format!("stdin read: {err}"))?;
    Ok(buf.lines().next().unwrap_or_default().to_string())
}

fn json_escape(src: &str) -> String {
    src.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn find_key(src: &str) -> String {
    let needle = "\"key\":\"";
    src.find(needle)
        .and_then(|start| src[start + needle.len()..].split('"').next())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("VERSION")
        .trim()
        .to_string()
}

fn read_os_release() -> Result<Vec<(String, String)>, String> {
    let src = std::fs::read_to_string("/etc/os-release").map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for line in src.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        if let Some((k, v)) = line.split_once('=') {
            out.push((k.trim().to_string(), v.trim().trim_matches('"').trim_matches('\'').to_string()));
        }
    }
    Ok(out)
}

fn doc() -> &'static str {
    r#"{"name":"hellodude","version":"0.1.0","author":"Gru","description":"Says hello and returns OS version from /etc/os-release.","arguments":[{"name":"key","type":"string","description":"A key inside the /etc/os-release file to retrieve (not used in this example). Default: VERSION","required":true}],"options":[{"name":"nohello","description":"Do not say hello"}],"examples":[{"description":"Get module output","code":"{ \"args\": { \"key\": \"VERSION\" } }"},{"description":"Get module documentation","code":"{ \"args\": { \"key\": \"VERSION\" }, \"opts\": [\"man\"] }"}],"returns":{"description":"Returns greeting and OS release info (if accessible).","sample":{"output":"hello, dude","os":{"PRETTY_NAME":"Debian GNU/Linux 12 (bookworm)","VERSION_ID":"12"}}}}"#
}

fn run(hdr: &str) -> String {
    let osr = match read_os_release() {
        Ok(data) => data,
        Err(err) => return format!("{{\"error\":\"failed to read /etc/os-release\",\"detail\":\"{}\"}}", json_escape(&err)),
    };

    let key = find_key(hdr);
    match osr.iter().find(|(k, _)| k == &key) {
        Some((_, val)) => {
            if has_opt(hdr, "nohello") {
                format!("{{\"{}\":\"{}\"}}", json_escape(&key), json_escape(val))
            } else {
                format!("{{\"{}\":\"{}\",\"output\":\"hello, dude\"}}", json_escape(&key), json_escape(val))
            }
        }
        None => format!("{{\"error\":\"unknown os-release key\",\"key\":\"{}\"}}", json_escape(&key)),
    }
}

fn main() {
    let hdr = match read_header() {
        Ok(hdr) => hdr,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    println!("{}", if has_opt(&hdr, "man") { doc().to_string() } else { run(&hdr) });
}
"##;

static CALLER_CARGO_TOML: &str = r#"
[package]
name = "caller"
version = "0.1.0"
edition = "2024"

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

static PKGAVAIL_CARGO_TOML: &str = r#"
[package]
name = "pkgavail"
version = "0.1.0"
edition = "2024"

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

static CALLER_MAIN_RS: &str = r##"
use std::io::{self, Read};

#[link(wasm_import_module = "api")]
unsafe extern "C" {
    #[link_name = "exec"]
    fn host_exec(req_ptr: u32, req_len: u32, out_ptr: u32, out_cap: u32) -> i32;
    #[link_name = "log"]
    fn host_log(level: i32, msg_ptr: u32, msg_len: u32);
}

fn has_opt(src: &str, want: &str) -> bool {
    src.contains(&format!("\"{want}\""))
}

fn read_header() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).map_err(|err| format!("stdin read: {err}"))?;
    Ok(buf.lines().next().unwrap_or_default().to_string())
}

fn json_escape(src: &str) -> String {
    src.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn log_info(msg: &str) {
    unsafe { host_log(1, msg.as_ptr() as u32, msg.len() as u32) };
}

fn exec_uname() -> Result<String, String> {
    let req = br#"{"argv":["/usr/bin/uname","-a"]}"#.to_vec();
    let mut out = vec![0u8; 256 * 1024];
    let n = unsafe { host_exec(req.as_ptr() as u32, req.len() as u32, out.as_mut_ptr() as u32, out.len() as u32) };
    if n < 0 {
        return Err(format!("host exec failed ({n})"));
    }

    let resp = String::from_utf8(out[..n as usize].to_vec()).map_err(|err| format!("bad host response json: {err}"))?;
    let code = if resp.contains("\"exit_code\":0") { 0 } else { 1 };
    let stdout = resp
        .split("\"stdout\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    let stderr = resp
        .split("\"stderr\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    if code != 0 {
        return Err(format!("exit {code}: {stderr}"));
    }
    Ok(stdout)
}

fn doc() -> &'static str {
    r#"{"name":"caller","version":"0.1.0","author":"Bo Maryniuk","description":"Executes `uname -a` via host syscall and returns stdout.","arguments":[],"options":[],"examples":[{"description":"Run uname","code":"{ \"args\": {} }"},{"description":"Show docs","code":"{ \"args\": {}, \"opts\": [\"man\"] }"}],"returns":{"description":"Returns stdout of uname -a","sample":{"output":"Linux host ..."}}}"#
}

fn main() {
    let hdr = match read_header() {
        Ok(hdr) => hdr,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    if has_opt(&hdr, "man") {
        println!("{}", doc());
        return;
    }

    log_info("Called: \"uname -a\"");
    match exec_uname() {
        Ok(out) => {
            println!("{{\"output\":\"{}\"}}", json_escape(out.trim_end()));
            log_info("Finished successfully");
        }
        Err(err) => println!("{{\"error\":\"{}\"}}", json_escape(&err)),
    }
}
"##;

static PKGAVAIL_MAIN_RS: &str = r##"
use std::io::{self, Read};

#[link(wasm_import_module = "api")]
unsafe extern "C" {
    #[link_name = "packagekit_available"]
    fn host_packagekit_available() -> i32;
    #[link_name = "packagekit_remove"]
    fn host_packagekit_remove(req_ptr: u32, req_len: u32, out_ptr: u32, out_cap: u32) -> i32;
    #[link_name = "packagekit_upgrade"]
    fn host_packagekit_upgrade(req_ptr: u32, req_len: u32, out_ptr: u32, out_cap: u32) -> i32;
}

fn has_opt(src: &str, want: &str) -> bool {
    src.contains(&format!("\"{want}\""))
}

fn read_header() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).map_err(|err| format!("stdin read: {err}"))?;
    Ok(buf.lines().next().unwrap_or_default().to_string())
}

fn doc() -> &'static str {
    r#"{"name":"pkgavail","version":"0.1.0","author":"Sysinspect","description":"Returns whether PackageKit helper is available in the host API.","arguments":[],"options":[],"returns":{"description":"Boolean PackageKit helper availability","sample":{"available":true}}}"#
}

fn main() {
    let hdr = match read_header() {
        Ok(hdr) => hdr,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    if has_opt(&hdr, "man") {
        println!("{}", doc());
        return;
    }

    let available = unsafe { host_packagekit_available() } != 0;
    let remove_rc = unsafe { host_packagekit_remove(0, 0, 0, 0) };
    let upgrade_rc = unsafe { host_packagekit_upgrade(0, 0, 0, 0) };
    println!(
        "{{\"available\":{},\"remove_import\":{},\"upgrade_import\":{}}}",
        if available { "true" } else { "false" },
        remove_rc,
        upgrade_rc
    );
}
"##;

static HOSTPEEK_CARGO_TOML: &str = r#"
[package]
name = "hostpeek"
version = "0.1.0"
edition = "2024"

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

static HOSTPEEK_MAIN_RS: &str = r##"
use std::io::{self, Read};

fn read_header() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).map_err(|err| format!("stdin read: {err}"))?;
    Ok(buf.lines().next().unwrap_or_default().to_string())
}

fn has_host(src: &str) -> bool {
    src.contains("\"host\":{") && src.contains("\"short\":\"wasm-host\"") && src.contains("\"sharelib\":\"/srv/wasm-share\"")
}

fn main() {
    let hdr = match read_header() {
        Ok(hdr) => hdr,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    println!("{{\"has_host\":{}}}", if has_host(&hdr) { "true" } else { "false" });
}
"##;

static ECHOREQ_CARGO_TOML: &str = r#"
[package]
name = "echoreq"
version = "0.1.0"
edition = "2024"

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

static ECHOREQ_MAIN_RS: &str = r##"
use std::io::{self, Read};

fn read_header() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).map_err(|err| format!("stdin read: {err}"))?;
    Ok(buf.lines().next().unwrap_or_default().to_string())
}

fn has(src: &str, needle: &str) -> bool {
    src.contains(needle)
}

fn main() {
    let hdr = match read_header() {
        Ok(hdr) => hdr,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    println!(
        "{{\"args_name\":{},\"args_enabled\":{},\"args_count\":{},\"opts_alpha\":{},\"opts_beta\":{},\"config_seen\":{},\"ext_seen\":{},\"host_trait_seen\":{},\"host_path_seen\":{},\"no_rt_mod\":{},\"no_rt_logs\":{}}}",
        if has(&hdr, "\"name\":\"Germany\"") { "true" } else { "false" },
        if has(&hdr, "\"enabled\":true") { "true" } else { "false" },
        if has(&hdr, "\"count\":7") { "true" } else { "false" },
        if has(&hdr, "\"alpha\"") { "true" } else { "false" },
        if has(&hdr, "\"beta\"") { "true" } else { "false" },
        if has(&hdr, "\"custom.flag\":\"seen\"") { "true" } else { "false" },
        if has(&hdr, "\"trace_id\":\"abc-123\"") && has(&hdr, "\"items\":[1,2,3]") { "true" } else { "false" },
        if has(&hdr, "\"system.hostname\":\"wasm-minion\"") { "true" } else { "false" },
        if has(&hdr, "\"sharelib\":\"/srv/wasm-share\"") { "true" } else { "false" },
        if !has(&hdr, "\"rt.mod\"") { "true" } else { "false" },
        if !has(&hdr, "\"rt.logs\"") { "true" } else { "false" }
    );
}
"##;

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

fn build_rust_example(example_dir: &Path, output_name: &str, bin_name: &str) -> PathBuf {
    let out = wasm_cache_dir().join(output_name);
    if out.exists() {
        return out;
    }
    if !example_dir.is_dir() {
        panic!("rust wasm example directory does not exist: {}", example_dir.display());
    }

    let target_dir = example_dir.join("target");
    let status = Command::new("cargo")
        .current_dir(example_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg("wasm32-wasip1")
        .status()
        .unwrap_or_else(|err| panic!("failed to run 'cargo build' in {}: {err}", example_dir.display()));
    if !status.success() {
        panic!("Rust wasm build failed in {} with status {}", example_dir.display(), status);
    }

    let built = target_dir.join("wasm32-wasip1/release").join(format!("{bin_name}.wasm"));
    fs::copy(&built, &out).unwrap_or_else(|err| panic!("failed to cache wasm module {}: {err}", built.display()));

    out
}

fn stage_rust_example(name: &str, files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("sysinspect-wasm-runtime-rust-{name}-"))
        .tempdir()
        .unwrap_or_else(|err| panic!("failed to create temporary Rust example directory: {err}"));

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
    let hello_src = stage_rust_example("hello", &[("Cargo.toml", HELLO_CARGO_TOML), ("src/main.rs", HELLO_MAIN_RS)]);
    let hello = build_rust_example(hello_src.path(), "hellodude.wasm", "hellodude");
    install_module(root, &hello, "hellodude.wasm");
    let caller_src = stage_rust_example("caller", &[("Cargo.toml", CALLER_CARGO_TOML), ("src/main.rs", CALLER_MAIN_RS)]);
    let caller = build_rust_example(caller_src.path(), "caller.wasm", "caller");
    install_module(root, &caller, "caller.wasm");
    let pkgavail_src = stage_rust_example("pkgavail", &[("Cargo.toml", PKGAVAIL_CARGO_TOML), ("src/main.rs", PKGAVAIL_MAIN_RS)]);
    let pkgavail = build_rust_example(pkgavail_src.path(), "pkgavail.wasm", "pkgavail");
    install_module(root, &pkgavail, "pkgavail.wasm");
    let echoreq_src = stage_rust_example("echoreq", &[("Cargo.toml", ECHOREQ_CARGO_TOML), ("src/main.rs", ECHOREQ_MAIN_RS)]);
    let echoreq = build_rust_example(echoreq_src.path(), "echoreq.wasm", "echoreq");
    install_module(root, &echoreq, "echoreq.wasm");
    let hostpeek_src = stage_rust_example("hostpeek", &[("Cargo.toml", HOSTPEEK_CARGO_TOML), ("src/main.rs", HOSTPEEK_MAIN_RS)]);
    let hostpeek = build_rust_example(hostpeek_src.path(), "hostpeek.wasm", "hostpeek");
    install_module(root, &hostpeek, "hostpeek.wasm");
    let mut modules = vec!["caller".to_string(), "echoreq".to_string(), "hellodude".to_string(), "hostpeek".to_string(), "pkgavail".to_string()];
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
fn test_wasm_runtime_exposes_packagekit_helper() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "opts": [],
        "args": { "rt.mod": "pkgavail" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert!(out.pointer("/data/available").is_some());
    assert!(out.pointer("/data/available").and_then(|v| v.as_bool()).is_some());
    assert_eq!(out.pointer("/data/remove_import"), Some(&json!(-2)));
    assert_eq!(out.pointer("/data/upgrade_import"), Some(&json!(-2)));
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
    assert!(logs.iter().any(|v| v.as_str().unwrap_or_default().contains("INFO")));
    assert!(logs.iter().any(|v| v.as_str().unwrap_or_default().contains("Called: \"uname -a\"")));
}

#[test]
fn test_wasm_runtime_passes_host_context_to_guest() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy() },
        "host": {
            "sys": { "hostname": { "short": "wasm-host" } },
            "paths": { "sharelib": "/srv/wasm-share" }
        },
        "opts": [],
        "args": { "rt.mod": "hostpeek" }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(out.pointer("/data/has_host"), Some(&json!(true)));
}

#[test]
fn test_wasm_runtime_preserves_request_sections_and_contract_shape() {
    let root = mk_tmp_runtime_root();
    install_test_modules(root.path());

    let out = run_runtime(&json!({
        "config": { "path.sharelib": root.path().to_string_lossy(), "custom.flag": "seen" },
        "host": {
            "traits": { "system.hostname": "wasm-minion" },
            "paths": { "sharelib": "/srv/wasm-share" }
        },
        "opts": ["alpha", "beta", "rt.logs"],
        "args": { "rt.mod": "echoreq", "name": "Germany", "enabled": true, "count": 7 },
        "trace_id": "abc-123",
        "payload": { "items": [1, 2, 3] }
    }));

    assert_eq!(out.get("retcode"), Some(&json!(0)));
    assert_eq!(
        out.get("data"),
        Some(&json!({
            "changed": false,
            "args_name": true,
            "args_enabled": true,
            "args_count": true,
            "opts_alpha": true,
            "opts_beta": true,
            "config_seen": true,
            "ext_seen": true,
            "host_trait_seen": true,
            "host_path_seen": true,
            "no_rt_mod": true,
            "no_rt_logs": true,
            "__sysinspect-module-logs": []
        }))
    );
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
