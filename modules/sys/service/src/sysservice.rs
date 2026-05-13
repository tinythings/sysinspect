use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

const EMBEDDED_YAML: &str = include_str!("service_managers.yaml");

/// Description of one service manager: its OS filter, detection command,
/// and templates for each supported operation.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ManagerDef {
    pub(crate) os: String,
    pub(crate) detect: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    pub(crate) start: String,
    pub(crate) stop: String,
    pub(crate) restart: String,
    #[serde(default)]
    pub(crate) reload: Option<String>,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) enable: Option<String>,
    #[serde(default)]
    pub(crate) disable: Option<String>,
}

/// Top-level YAML structure: a map of manager IDs to their definitions.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    pub(crate) managers: HashMap<String, ManagerDef>,
}

impl Config {
    pub(crate) fn from_merged(user_yaml: Option<&str>) -> Result<Self, String> {
        let mut builtin: Config = serde_yaml::from_str(EMBEDDED_YAML).map_err(|e| format!("Failed to parse built-in YAML: {e}"))?;
        if let Some(user) = user_yaml {
            let user_cfg: Config = serde_yaml::from_str(user).map_err(|e| format!("Failed to parse user YAML: {e}"))?;
            for (id, mgr) in user_cfg.managers {
                builtin.managers.insert(id, mgr);
            }
        }
        Ok(builtin)
    }

    /// Return the first manager matching the current OS whose `detect`
    /// command exits successfully.
    pub(crate) fn detect(&self) -> Option<(&str, &ManagerDef)> {
        let os = current_os();
        self.managers
            .iter()
            .filter(|(_, m)| m.os == os)
            .find(|(_, m)| Command::new("sh").arg("-c").arg(&m.detect).output().map(|o| o.status.success()).unwrap_or(false))
            .map(|(id, m)| (id.as_str(), m))
    }
}

/// Map the compile-time target OS to the string key used in the YAML file.
pub(crate) fn current_os() -> &'static str {
    if cfg!(target_os = "freebsd") {
        "freebsd"
    } else if cfg!(target_os = "openbsd") {
        "openbsd"
    } else if cfg!(target_os = "netbsd") {
        "netbsd"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        ""
    }
}

/// Main entry point called by the module runtime. Parses the request,
/// detects the service manager, executes the operation, and always
/// returns telemetry data suitable for model-level conditions and
/// OpenTelemetry forwarding.
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut resp = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");
    let dry_run = runtime::get_opt(rt, "dry-run");

    if name.is_empty() {
        resp.set_retcode(1);
        resp.set_message("Argument \"name\" is required");
        return resp;
    }

    let sharelib = rt.config().get("path.sharelib").and_then(|v| v.as_string()).unwrap_or_else(|| "/usr/share/sysinspect".to_string());
    let lib_path = format!("{}/lib/sys.service.yaml", sharelib.trim_end_matches('/'));
    let user_yaml = std::fs::read_to_string(&lib_path).ok();

    let service_config = match Config::from_merged(user_yaml.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            resp.set_retcode(1);
            resp.set_message(&e);
            return resp;
        }
    };

    let op = match parse_operation(rt, &mut resp) {
        Some(o) => o,
        None => return resp,
    };

    let (mgr_id, mgr) = match service_config.detect() {
        Some(m) => m,
        None => {
            resp.set_retcode(1);
            resp.set_message(&format!("No service manager detected for OS '{}'", current_os()));
            return resp;
        }
    };

    let op_template = match resolve_template(mgr, op) {
        Some(t) => t,
        None => {
            resp.set_retcode(1);
            resp.set_message(&format!("Operation '{}' is not supported by manager '{}'", op, mgr_id));
            return resp;
        }
    };

    let cmd = op_template.replace("{name}", &name);

    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] {cmd}"));
        let mut data = telemetry_base(&name, mgr_id);
        data.insert("command".to_string(), serde_json::Value::String(cmd));
        if let Err(e) = resp.set_data(&data) {
            resp.add_warning(&format!("{e}"));
        }
        return resp;
    }

    match exec_sh(&cmd) {
        Ok((code, stdout, stderr)) => {
            let alive = if op == "status" { parse_status_output(mgr_id, &stdout) } else { code == 0 };
            let pids = if alive { find_pids(&name) } else { Vec::new() };

            if op == "check" {
                resp.set_retcode(0);
            } else if op == "status" {
                resp.set_retcode(if alive { 0 } else { 1 });
            } else {
                resp.set_retcode(code);
            }

            if op == "check" || op == "status" {
                resp.set_message(&format!("Service '{}' is {}", name, if alive { "running" } else { "not running" }));
            } else {
                resp.set_message(&format!("Service '{}' {} {}", name, op, if code == 0 { "successful" } else { "failed" }));
            }

            let mut data = telemetry_base(&name, mgr_id);
            data.insert("running".to_string(), serde_json::Value::Bool(alive));
            data.insert("exit_code".to_string(), serde_json::Value::Number(serde_json::Number::from(code)));
            if !pids.is_empty() {
                data.insert(
                    "pids".to_string(),
                    serde_json::Value::Array(pids.iter().map(|p| serde_json::Value::Number(serde_json::Number::from(*p))).collect()),
                );
            }
            if !stdout.is_empty() {
                data.insert("stdout".to_string(), serde_json::Value::String(stdout.trim().to_string()));
            }
            if !stderr.is_empty() {
                data.insert("stderr".to_string(), serde_json::Value::String(stderr.trim().to_string()));
            }
            if let Err(e) = resp.set_data(&data) {
                resp.add_warning(&format!("{e}"));
            }
        }
        Err(e) => {
            resp.set_retcode(1);
            resp.set_message(&e);
            let mut data = telemetry_base(&name, mgr_id);
            data.insert("running".to_string(), serde_json::Value::Bool(false));
            if let Err(e) = resp.set_data(&data) {
                resp.add_warning(&format!("{e}"));
            }
        }
    }

    resp
}

/// Build the common telemetry fields returned by every operation.
pub(crate) fn telemetry_base(name: &str, mgr_id: &str) -> HashMap<String, serde_json::Value> {
    let mut m = HashMap::new();
    m.insert("name".to_string(), serde_json::Value::String(name.to_string()));
    m.insert("manager".to_string(), serde_json::Value::String(mgr_id.to_string()));
    m
}

/// Try to find PIDs for a service name using pgrep.
fn find_pids(name: &str) -> Vec<i32> {
    match Command::new("pgrep").arg("-x").arg(name).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse::<i32>().ok()).collect(),
        _ => Vec::new(),
    }
}

/// Interpret status output for the given manager.
/// Systemd relays its answer through the exit code (already checked);
/// rc.d and friends print "is running" in their stdout.
pub(crate) fn parse_status_output(mgr_id: &str, stdout: &str) -> bool {
    if mgr_id.contains("systemd") {
        return true;
    }
    stdout.contains("is running")
}

/// Resolve an operation name to the manager's command template.
pub(crate) fn resolve_template<'a>(mgr: &'a ManagerDef, op: &str) -> Option<&'a str> {
    match op {
        "start" => Some(mgr.start.as_str()),
        "stop" => Some(mgr.stop.as_str()),
        "restart" => Some(mgr.restart.as_str()),
        "reload" => mgr.reload.as_deref(),
        "status" | "check" => Some(mgr.status.as_str()),
        "enable" => mgr.enable.as_deref(),
        "disable" => mgr.disable.as_deref(),
        _ => None,
    }
}

/// Extract the operation from the request options.
pub(crate) fn parse_operation(rt: &ModRequest, resp: &mut ModResponse) -> Option<&'static str> {
    if runtime::get_opt(rt, "check") {
        Some("check")
    } else if runtime::get_opt(rt, "status") {
        Some("status")
    } else if runtime::get_opt(rt, "start") {
        Some("start")
    } else if runtime::get_opt(rt, "stop") {
        Some("stop")
    } else if runtime::get_opt(rt, "restart") {
        Some("restart")
    } else if runtime::get_opt(rt, "reload") {
        Some("reload")
    } else if runtime::get_opt(rt, "enable") {
        Some("enable")
    } else if runtime::get_opt(rt, "disable") {
        Some("disable")
    } else {
        resp.set_retcode(1);
        resp.set_message("No operation specified. Use --check, --start, --stop, --restart, --reload, --enable, --disable, or --status");
        None
    }
}

/// Run a shell command, returning the exit code, stdout, and stderr.
fn exec_sh(cmd: &str) -> Result<(i32, String, String), String> {
    let child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute: {e}"))?;

    let out = child.wait_with_output().map_err(|e| format!("Failed to wait: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((out.status.code().unwrap_or(1), stdout, stderr))
}
