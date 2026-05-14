use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

const EMBEDDED_YAML: &str = include_str!("kernel_managers.yaml");

/// Description of one kernel module manager: OS filter, detection,
/// and command templates for each operation.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ManagerDef {
    pub(crate) os: String,
    pub(crate) detect: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    pub(crate) load: String,
    pub(crate) unload: String,
    pub(crate) status: String,
    pub(crate) info: String,
    pub(crate) list_modules: String,
}

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

    pub(crate) fn detect(&self) -> Option<(&str, &ManagerDef)> {
        let os = current_os();
        self.managers
            .iter()
            .filter(|(_, m)| m.os == os)
            .find(|(_, m)| Command::new("sh").arg("-c").arg(&m.detect).output().map(|o| o.status.success()).unwrap_or(false))
            .map(|(id, m)| (id.as_str(), m))
    }
}

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
    } else if cfg!(target_os = "solaris") {
        "solaris"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        ""
    }
}

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut resp = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");
    let dry_run = runtime::get_opt(rt, "dry-run");

    let sharelib = rt.config().get("path.sharelib").and_then(|v| v.as_string()).unwrap_or_else(|| "/usr/share/sysinspect".to_string());
    let lib_path = format!("{}/lib/os.kernel.yaml", sharelib.trim_end_matches('/'));
    let user_yaml = std::fs::read_to_string(&lib_path).ok();

    let cfg = match Config::from_merged(user_yaml.as_deref()) {
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

    if dry_run {
        let (mgr_hint, maybe_cmd) = match cfg.detect() {
            Some(m) => {
                let tmpl = if op == "list" { m.1.list_modules.as_str() } else { resolve_template(m.1, op) };
                (m.0.to_string(), Some(tmpl.replace("{name}", &name)))
            }
            None => ("<none>".to_string(), None),
        };
        resp.set_retcode(0);
        if let Some(c) = maybe_cmd {
            resp.set_message(&format!("[dry-run] manager={mgr_hint} cmd={c}"));
        } else {
            resp.set_message(&format!("[dry-run] no kernel module manager detected for OS '{}'", current_os()));
        }
        return resp;
    }

    let (mgr_id, mgr) = match cfg.detect() {
        Some(m) => m,
        None => {
            resp.set_retcode(1);
            resp.set_message(&format!("No kernel module manager detected for OS '{}'", current_os()));
            return resp;
        }
    };

    if op == "list" {
        return run_list(&mgr.list_modules, mgr_id, dry_run, &mut resp);
    }

    if name.is_empty() {
        resp.set_retcode(1);
        resp.set_message("Argument \"name\" is required for this operation");
        return resp;
    }

    let template = resolve_template(mgr, op);
    let cmd = template.replace("{name}", &name);

    if op == "status" || op == "info" {
        return run_inspect(&cmd, mgr_id, &name, op, mgr, &mut resp);
    }

    if op == "load" {
        let status_cmd = mgr.status.replace("{name}", &name);
        if let Ok((0, _, _)) = exec_sh(&status_cmd) {
            resp.set_retcode(0);
            resp.set_message(&format!("Kernel module '{}' is already loaded", name));
            return resp;
        }
    }
    if op == "unload" {
        let status_cmd = mgr.status.replace("{name}", &name);
        if let Ok((1, _, _)) = exec_sh(&status_cmd) {
            resp.set_retcode(0);
            resp.set_message(&format!("Kernel module '{}' is not loaded", name));
            return resp;
        }
    }

    match exec_sh(&cmd) {
        Ok((code, stdout, stderr)) => {
            resp.set_retcode(code);
            resp.set_message(&format!("Kernel module '{}' {} {}", name, op, if code == 0 { "successful" } else { "failed" }));
            let mut data = telemetry_base(&name, mgr_id);
            data.insert("exit_code".to_string(), serde_json::Value::Number(serde_json::Number::from(code)));
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
        }
    }

    resp
}

fn run_list(list_cmd: &str, mgr_id: &str, dry_run: bool, resp: &mut ModResponse) -> ModResponse {
    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] {list_cmd}"));
        return resp.clone();
    }

    match exec_sh(list_cmd) {
        Ok((0, stdout, _)) => {
            resp.set_retcode(0);
            resp.set_message("Kernel modules listed");
            let modules = parse_list_output(mgr_id, &stdout);
            let mut data = HashMap::new();
            data.insert("manager".to_string(), serde_json::Value::String(mgr_id.to_string()));
            data.insert("modules".to_string(), serde_json::Value::Array(modules));
            if let Err(e) = resp.set_data(&data) {
                resp.add_warning(&format!("{e}"));
            }
        }
        Ok((code, _, _)) => {
            resp.set_retcode(code);
            resp.set_message("Failed to list kernel modules");
        }
        Err(e) => {
            resp.set_retcode(1);
            resp.set_message(&e);
        }
    }
    resp.clone()
}

pub(crate) fn parse_list_output(mgr_id: &str, stdout: &str) -> Vec<serde_json::Value> {
    if mgr_id.contains("kld") {
        parse_kldstat_list(stdout)
    } else if mgr_id.contains("linux") || mgr_id.contains("android") {
        parse_lsmod_list(stdout)
    } else if mgr_id.contains("kext") {
        parse_kextstat_list(stdout)
    } else if mgr_id.contains("netbsd") || mgr_id.contains("solaris") {
        parse_modstat_list(stdout)
    } else {
        Vec::new()
    }
}

fn parse_kldstat_list(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 5 && parts[0].parse::<u32>().is_ok() {
                let mut m = HashMap::new();
                m.insert("name".to_string(), serde_json::Value::String(parts[4].to_string()));
                m.insert("loaded".to_string(), serde_json::Value::Bool(true));
                m.insert("id".to_string(), serde_json::Value::String(parts[0].to_string()));
                m.insert("refs".to_string(), serde_json::Value::String(parts[1].to_string()));
                if let Ok(size) = parts[3].parse::<u64>() {
                    m.insert("size_bytes".to_string(), serde_json::Value::Number(serde_json::Number::from(size)));
                }
                Some(serde_json::Value::Object(m.into_iter().collect()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_lsmod_list(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .skip(1)
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 3 {
                let mut m = HashMap::new();
                m.insert("name".to_string(), serde_json::Value::String(parts[0].to_string()));
                m.insert("loaded".to_string(), serde_json::Value::Bool(true));
                if let Ok(size) = parts[1].parse::<u64>() {
                    m.insert("size_bytes".to_string(), serde_json::Value::Number(serde_json::Number::from(size)));
                }
                Some(serde_json::Value::Object(m.into_iter().collect()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_kextstat_list(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .skip(1)
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 6 {
                let mut m = HashMap::new();
                m.insert("name".to_string(), serde_json::Value::String(parts[5].to_string()));
                m.insert("loaded".to_string(), serde_json::Value::Bool(true));
                m.insert("id".to_string(), serde_json::Value::String(parts[0].to_string()));
                Some(serde_json::Value::Object(m.into_iter().collect()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_modstat_list(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 3 {
                let mut m = HashMap::new();
                m.insert("name".to_string(), serde_json::Value::String(parts[0].to_string()));
                m.insert("loaded".to_string(), serde_json::Value::Bool(true));
                Some(serde_json::Value::Object(m.into_iter().collect()))
            } else {
                None
            }
        })
        .collect()
}

fn run_inspect(cmd: &str, mgr_id: &str, name: &str, op: &str, _mgr: &ManagerDef, resp: &mut ModResponse) -> ModResponse {
    match exec_sh(cmd) {
        Ok((code, stdout, _)) => {
            let loaded = code == 0;
            resp.set_retcode(if op == "info" || loaded { 0 } else { 1 });
            resp.set_message(&format!("Kernel module '{}' is {}", name, if loaded { "loaded" } else { "not loaded" }));

            let mut data = telemetry_base(name, mgr_id);
            data.insert("loaded".to_string(), serde_json::Value::Bool(loaded));
            data.insert("exit_code".to_string(), serde_json::Value::Number(serde_json::Number::from(code)));
            if !stdout.is_empty() {
                data.insert("stdout".to_string(), serde_json::Value::String(stdout.clone()));
            }
            if op == "info" && loaded {
                parse_info_output(mgr_id, &stdout, &mut data);
            }
            if let Err(e) = resp.set_data(&data) {
                resp.add_warning(&format!("{e}"));
            }
        }
        Err(e) => {
            resp.set_retcode(if op == "info" { 0 } else { 1 });
            resp.set_message(&e);
        }
    }
    resp.clone()
}

fn parse_info_output(mgr_id: &str, stdout: &str, data: &mut HashMap<String, serde_json::Value>) {
    if mgr_id.contains("kld") {
        parse_kldstat_info(stdout, data);
    } else if mgr_id.contains("linux") || mgr_id.contains("android") {
        parse_modinfo(stdout, data);
    }
}

fn parse_kldstat_info(stdout: &str, data: &mut HashMap<String, serde_json::Value>) {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains(".ko")
            && let Some(path) = trimmed.split_whitespace().last()
        {
            data.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        }
    }
}

fn parse_modinfo(stdout: &str, data: &mut HashMap<String, serde_json::Value>) {
    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("filename:") {
            data.insert("path".to_string(), serde_json::Value::String(v.trim().to_string()));
        } else if let Some(v) = line.strip_prefix("version:") {
            data.insert("version".to_string(), serde_json::Value::String(v.trim().to_string()));
        } else if let Some(v) = line.strip_prefix("description:") {
            data.insert("description".to_string(), serde_json::Value::String(v.trim().to_string()));
        } else if let Some(v) = line.strip_prefix("author:") {
            data.insert("author".to_string(), serde_json::Value::String(v.trim().to_string()));
        } else if let Some(v) = line.strip_prefix("license:") {
            data.insert("license".to_string(), serde_json::Value::String(v.trim().to_string()));
        } else if let Some(v) = line.strip_prefix("depends:") {
            let deps: Vec<_> = v.split(',').map(|d| serde_json::Value::String(d.trim().to_string())).collect();
            data.insert("dependencies".to_string(), serde_json::Value::Array(deps));
        } else if let Some(v) = line.strip_prefix("srcversion:") {
            data.insert("srcversion".to_string(), serde_json::Value::String(v.trim().to_string()));
        }
    }
}

pub(crate) fn telemetry_base(name: &str, mgr_id: &str) -> HashMap<String, serde_json::Value> {
    let mut m = HashMap::new();
    m.insert("name".to_string(), serde_json::Value::String(name.to_string()));
    m.insert("manager".to_string(), serde_json::Value::String(mgr_id.to_string()));
    m
}

pub(crate) fn resolve_template<'a>(mgr: &'a ManagerDef, op: &str) -> &'a str {
    match op {
        "load" => mgr.load.as_str(),
        "unload" => mgr.unload.as_str(),
        "status" => mgr.status.as_str(),
        "info" => mgr.info.as_str(),
        _ => mgr.status.as_str(),
    }
}

pub(crate) fn parse_operation(rt: &ModRequest, resp: &mut ModResponse) -> Option<&'static str> {
    if runtime::get_opt(rt, "status") {
        Some("status")
    } else if runtime::get_opt(rt, "info") {
        Some("info")
    } else if runtime::get_opt(rt, "list") {
        Some("list")
    } else if runtime::get_opt(rt, "load") {
        Some("load")
    } else if runtime::get_opt(rt, "unload") {
        Some("unload")
    } else {
        resp.set_retcode(1);
        resp.set_message("No operation specified. Use --status, --info, --list, --load, or --unload");
        None
    }
}

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
