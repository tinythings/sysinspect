use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

// ---------------------------------------------------------------------------
// Inspected package state returned by --check and used internally
// ---------------------------------------------------------------------------
#[derive(Debug, Default)]
struct PkgState {
    installed: bool,
    installed_version: Option<String>,
    available_version: Option<String>,
    upgradable: bool,
    repo: Option<String>,
}

impl PkgState {
    fn to_data(&self, name: &str) -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert("name".to_string(), Value::String(name.to_string()));
        m.insert("installed".to_string(), Value::Bool(self.installed));
        if let Some(ref v) = self.installed_version {
            m.insert("installed_version".to_string(), Value::String(v.clone()));
        }
        if let Some(ref v) = self.available_version {
            m.insert("available_version".to_string(), Value::String(v.clone()));
        }
        m.insert("upgradable".to_string(), Value::Bool(self.upgradable));
        if let Some(ref r) = self.repo {
            m.insert("repo".to_string(), Value::String(r.clone()));
        }
        m
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut response = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");
    let dry_run = runtime::get_opt(rt, "dry-run");
    let force = runtime::get_opt(rt, "force");

    let op = get_operation(rt, &mut response);
    if op.is_empty() {
        return response;
    }

    // ------- check: pure inspection, no action ----------
    if op == "check" {
        return run_check(&name, dry_run, &mut response);
    }

    // ------- search: just query, no inspection needed -----
    if op == "search" || op == "update" {
        return run_mutation(&op, &name, dry_run, &mut response);
    }

    // ------- install / remove / upgrade: inspect first ------
    if !force && !name.is_empty() {
        let state = check_package(&name);
        match op.as_str() {
            "install"
                if state.installed => {
                    response.set_retcode(0);
                    response.set_message(&format!("Package '{}' is already installed", name));
                    if let Err(e) = response.set_data(state.to_data(&name)) {
                        response.add_warning(&format!("{e}"));
                    }
                    return response;
                }
            "remove"
                if !state.installed => {
                    response.set_retcode(0);
                    response.set_message(&format!("Package '{}' is not installed", name));
                    if let Err(e) = response.set_data(state.to_data(&name)) {
                        response.add_warning(&format!("{e}"));
                    }
                    return response;
                }
            "upgrade"
                if !state.upgradable => {
                    response.set_retcode(0);
                    response.set_message(&format!("Package '{}' is already up to date", name));
                    if let Err(e) = response.set_data(state.to_data(&name)) {
                        response.add_warning(&format!("{e}"));
                    }
                    return response;
                }
            _ => {}
        }
    }

    if op == "upgrade" && name.is_empty() {
        // bulk upgrade: check if anything is upgradable at all
        let upgradable = list_upgradable();
        if upgradable.is_empty() {
            response.set_retcode(0);
            response.set_message("All packages are up to date");
            return response;
        }
        response.add_warning(&format!("Upgrading {} package(s): {}", upgradable.len(), upgradable.join(", ")));
    }

    run_mutation(&op, &name, dry_run, &mut response)
}

// ---------------------------------------------------------------------------
// Operations that just run a command (check handled separately)
// ---------------------------------------------------------------------------
fn run_mutation(op: &str, name: &str, dry_run: bool, response: &mut ModResponse) -> ModResponse {
    match get_pkg_command(op, name) {
        Ok((cmd, args)) => {
            if dry_run {
                response.set_retcode(0);
                response.set_message(&format!("[dry-run] {} {}", cmd, args.join(" ")));
                return response.clone();
            }

            match exec(&cmd, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
                Ok((code, stdout, stderr)) => {
                    response.set_retcode(code);
                    if code == 0 {
                        response.set_message(&format!("Package operation '{}' completed", op));
                    } else {
                        response.set_message(&format!("Package operation '{}' failed: {}", op, stderr.trim()));
                    }
                    if !stdout.is_empty() {
                        response.add_warning(stdout.trim());
                    }
                }
                Err(err) => {
                    response.set_retcode(1);
                    response.set_message(&err);
                }
            }
        }
        Err(err) => {
            response.set_retcode(1);
            response.set_message(&err);
        }
    }
    response.clone()
}

// ---------------------------------------------------------------------------
// Check / inspect
// ---------------------------------------------------------------------------
fn run_check(name: &str, dry_run: bool, response: &mut ModResponse) -> ModResponse {
    if name.is_empty() {
        response.set_retcode(1);
        response.set_message("--check requires a package --name");
        return response.clone();
    }

    if dry_run {
        let (cmd, args) = match get_check_command(name) {
            Ok(c) => c,
            Err(e) => {
                response.set_retcode(1);
                response.set_message(&e);
                return response.clone();
            }
        };
        response.set_retcode(0);
        response.set_message(&format!("[dry-run] {} {}", cmd, args.join(" ")));
        return response.clone();
    }

    let state = check_package(name);
    response.set_retcode(0);
    if state.installed {
        if state.upgradable {
            response.set_message(&format!(
                "Package '{}' is installed (v{}) — upgrade available (v{})",
                name,
                state.installed_version.as_deref().unwrap_or("?"),
                state.available_version.as_deref().unwrap_or("?")
            ));
        } else {
            response.set_message(&format!(
                "Package '{}' is installed (v{}) — up to date",
                name,
                state.installed_version.as_deref().unwrap_or("?")
            ));
        }
    } else {
        response.set_message(&format!("Package '{}' is not installed", name));
    }
    if let Err(e) = response.set_data(state.to_data(name)) {
        response.add_warning(&format!("{e}"));
    }
    response.clone()
}

// ---------------------------------------------------------------------------
// Operation parser
// ---------------------------------------------------------------------------
pub(crate) fn get_operation(rt: &ModRequest, response: &mut ModResponse) -> String {
    if runtime::get_opt(rt, "check") {
        "check".to_string()
    } else if runtime::get_opt(rt, "install") {
        "install".to_string()
    } else if runtime::get_opt(rt, "remove") {
        "remove".to_string()
    } else if runtime::get_opt(rt, "update") {
        "update".to_string()
    } else if runtime::get_opt(rt, "upgrade") {
        "upgrade".to_string()
    } else if runtime::get_opt(rt, "search") {
        "search".to_string()
    } else {
        response.set_retcode(1);
        response.set_message("No operation specified. Use --check, --install, --remove, --update, --upgrade, or --search");
        String::new()
    }
}

// ===================================================================
// OS-specific: package manager commands for mutation
// ===================================================================

#[cfg(target_os = "freebsd")]
pub(crate) fn get_pkg_command(op: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let args = match op {
        "install" => vec!["install".to_string(), "-y".to_string(), name.to_string()],
        "remove" => vec!["delete".to_string(), "-y".to_string(), name.to_string()],
        "update" => vec!["update".to_string()],
        "upgrade" => vec!["upgrade".to_string(), "-y".to_string()],
        "search" => vec!["search".to_string(), name.to_string()],
        _ => return Err(format!("Unknown operation: {op}")),
    };
    Ok(("pkg".to_string(), args))
}

#[cfg(target_os = "openbsd")]
pub(crate) fn get_pkg_command(op: &str, name: &str) -> Result<(String, Vec<String>), String> {
    match op {
        "install" => Ok(("pkg_add".to_string(), vec![name.to_string()])),
        "remove" => Ok(("pkg_delete".to_string(), vec![name.to_string()])),
        "update" => Err("OpenBSD pkg_add has no repository update; packages are fetched directly".to_string()),
        "upgrade" => Ok(("pkg_add".to_string(), vec!["-u".to_string(), name.to_string()])),
        "search" => Ok(("pkg_info".to_string(), vec!["-Q".to_string(), name.to_string()])),
        _ => Err(format!("Unknown operation: {op}")),
    }
}

#[cfg(target_os = "netbsd")]
pub(crate) fn get_pkg_command(op: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let has_pkgin = Command::new("pkgin")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_pkgin {
        match op {
            "install" => Ok(("pkgin".to_string(), vec!["-y".to_string(), "install".to_string(), name.to_string()])),
            "remove" => Ok(("pkgin".to_string(), vec!["-y".to_string(), "remove".to_string(), name.to_string()])),
            "update" => Ok(("pkgin".to_string(), vec!["-y".to_string(), "update".to_string()])),
            "upgrade" => Ok(("pkgin".to_string(), vec!["-y".to_string(), "upgrade".to_string()])),
            "search" => Ok(("pkgin".to_string(), vec!["search".to_string(), name.to_string()])),
            _ => Err(format!("Unknown operation: {op}")),
        }
    } else {
        match op {
            "install" => Ok(("pkg_add".to_string(), vec![name.to_string()])),
            "remove" => Ok(("pkg_delete".to_string(), vec![name.to_string()])),
            "update" => Err("NetBSD pkg_add has no repository update command".to_string()),
            "upgrade" => Ok(("pkg_add".to_string(), vec!["-u".to_string(), name.to_string()])),
            "search" => Err("NetBSD: install pkgin for search support".to_string()),
            _ => Err(format!("Unknown operation: {op}")),
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn get_pkg_command(op: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let has_brew = Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_brew {
        match op {
            "install" => Ok(("brew".to_string(), vec!["install".to_string(), name.to_string()])),
            "remove" => Ok(("brew".to_string(), vec!["uninstall".to_string(), name.to_string()])),
            "update" => Ok(("brew".to_string(), vec!["update".to_string()])),
            "upgrade" => Ok(("brew".to_string(), vec!["upgrade".to_string()])),
            "search" => Ok(("brew".to_string(), vec!["search".to_string(), name.to_string()])),
            _ => Err(format!("Unknown operation: {op}")),
        }
    } else {
        Err("No supported package manager found on macOS (brew not detected)".to_string())
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn detect_linux_pkg_manager() -> Option<&'static str> {
    let candidates: &[&str] = &["apt-get", "dnf", "yum", "zypper", "pacman", "apk"];

    candidates.iter().find_map(|bin| {
        Command::new(bin)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|_| *bin)
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn get_pkg_command(op: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let bin = detect_linux_pkg_manager()
        .ok_or_else(|| "No supported package manager found (tried: apt-get, dnf, yum, zypper, pacman, apk)".to_string())?;

    let args = match (bin, op) {
        ("apt-get", "install") => vec!["install".to_string(), "-y".to_string(), name.to_string()],
        ("apt-get", "remove") => vec!["remove".to_string(), "-y".to_string(), name.to_string()],
        ("apt-get", "update") => vec!["update".to_string()],
        ("apt-get", "upgrade") => vec!["upgrade".to_string(), "-y".to_string()],
        ("apt-get", "search") => return Ok(("apt-cache".to_string(), vec!["search".to_string(), name.to_string()])),

        ("dnf" | "yum", "install") => vec!["install".to_string(), "-y".to_string(), name.to_string()],
        ("dnf" | "yum", "remove") => vec!["remove".to_string(), "-y".to_string(), name.to_string()],
        ("dnf" | "yum", "update") => vec!["makecache".to_string()],
        ("dnf" | "yum", "upgrade") => vec!["upgrade".to_string(), "-y".to_string()],
        ("dnf" | "yum", "search") => vec!["search".to_string(), name.to_string()],

        ("zypper", "install") => vec!["install".to_string(), "-y".to_string(), name.to_string()],
        ("zypper", "remove") => vec!["remove".to_string(), "-y".to_string(), name.to_string()],
        ("zypper", "update") => vec!["refresh".to_string()],
        ("zypper", "upgrade") => vec!["update".to_string(), "-y".to_string()],
        ("zypper", "search") => vec!["search".to_string(), name.to_string()],

        ("pacman", "install") => vec!["-S".to_string(), "--noconfirm".to_string(), name.to_string()],
        ("pacman", "remove") => vec!["-R".to_string(), "--noconfirm".to_string(), name.to_string()],
        ("pacman", "update") => vec!["-Sy".to_string()],
        ("pacman", "upgrade") => vec!["-Su".to_string(), "--noconfirm".to_string()],
        ("pacman", "search") => vec!["-Ss".to_string(), name.to_string()],

        ("apk", "install") => vec!["add".to_string(), name.to_string()],
        ("apk", "remove") => vec!["del".to_string(), name.to_string()],
        ("apk", "update") => vec!["update".to_string()],
        ("apk", "upgrade") => vec!["upgrade".to_string()],
        ("apk", "search") => vec!["search".to_string(), name.to_string()],

        _ => return Err(format!("Unsupported operation '{}' for {}", op, bin)),
    };

    Ok((bin.to_string(), args))
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "macos"
)))]
pub(crate) fn get_pkg_command(_op: &str, _name: &str) -> Result<(String, Vec<String>), String> {
    Err("Unsupported operating system".to_string())
}

// ===================================================================
// OS-specific: check / inspect
// ===================================================================

#[cfg(target_os = "freebsd")]
fn check_package(name: &str) -> PkgState {
    let mut state = PkgState::default();

    // Is it installed?
    if let Ok((0, stdout, _)) = exec("pkg", &["info", name]) {
        state.installed = true;
        // Parse installed version: "name-version" is the first token on first real line
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("Name") {
                if let Some(ver) = trimmed.split_whitespace().next() {
                    if let Some(v) = ver.strip_prefix(&format!("{}-", name)) {
                        state.installed_version = Some(v.to_string());
                    }
                }
                break;
            }
        }
    }

    // Available version & repo
    if let Ok((0, stdout, _)) = exec("pkg", &["rquery", "%v\t%R", name]) {
        if let Some(line) = stdout.lines().next() {
            let parts: Vec<&str> = line.split('\t').collect();
            if let Some(v) = parts.first() {
                state.available_version = Some(v.to_string());
            }
            if parts.len() > 1 {
                state.repo = Some(parts[1].to_string());
            }
        }
    }

    // Is it upgradable?
    if state.installed {
        if let Ok((0, stdout, _)) = exec("pkg", &["version", "-vL=", name]) {
            // Output like "nginx-1.24.0 < needs updating (repository has 1.26.0)"
            for line in stdout.lines() {
                if line.contains('<') && line.contains(name) {
                    state.upgradable = true;
                    break;
                }
            }
        }
    }

    state
}

#[cfg(target_os = "freebsd")]
fn list_upgradable() -> Vec<String> {
    if let Ok((0, stdout, _)) = exec("pkg", &["version", "-vL="]) {
        stdout
            .lines()
            .filter(|l| l.contains('<'))
            .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "freebsd")]
fn get_check_command(name: &str) -> Result<(String, Vec<String>), String> {
    Ok(("pkg".to_string(), vec!["info".to_string(), name.to_string()]))
}

// ---------- OpenBSD ----------

#[cfg(target_os = "openbsd")]
fn check_package(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, stdout, _)) = exec("pkg_info", &["-e", name]) {
        if !stdout.trim().is_empty() {
            state.installed = true;
        }
    }
    if state.installed {
        if let Ok((0, stdout, _)) = exec("pkg_info", &[name]) {
            for line in stdout.lines() {
                if line.starts_with("Information for") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(&ver) = parts.get(1) {
                    state.installed_version = Some(ver.to_string());
                }
                break;
            }
        }
    }
    // OpenBSD has no simple remote version check — skip available_version
    state
}

#[cfg(target_os = "openbsd")]
fn list_upgradable() -> Vec<String> {
    Vec::new() // pkg_add -u -n would list upgradable but returns non-trivial output
}

#[cfg(target_os = "openbsd")]
fn get_check_command(name: &str) -> Result<(String, Vec<String>), String> {
    Ok(("pkg_info".to_string(), vec![name.to_string()]))
}

// ---------- NetBSD ----------

#[cfg(target_os = "netbsd")]
fn check_package(name: &str) -> PkgState {
    let mut state = PkgState::default();
    let has_pkgin = Command::new("pkgin").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);

    if has_pkgin {
        if let Ok((0, stdout, _)) = exec("pkgin", &["list"]) {
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.first().map(|p| p.starts_with(name)).unwrap_or(false) {
                    state.installed = true;
                    if let Some(ver) = parts.get(1) {
                        state.installed_version = Some(ver.to_string());
                    }
                    break;
                }
            }
        }
    } else {
        if let Ok((0, _, _)) = exec("pkg_info", &["-e", name]) {
            state.installed = true;
        }
    }
    state
}

#[cfg(target_os = "netbsd")]
fn list_upgradable() -> Vec<String> {
    let has_pkgin = Command::new("pkgin").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if has_pkgin {
        if let Ok((0, stdout, _)) = exec("pkgin", &["upgrade", "-n"]) {
            return stdout.lines().filter_map(|l| l.split_whitespace().next().map(|s| s.to_string())).collect();
        }
    }
    Vec::new()
}

#[cfg(target_os = "netbsd")]
fn get_check_command(name: &str) -> Result<(String, Vec<String>), String> {
    let has_pkgin = Command::new("pkgin").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if has_pkgin {
        Ok(("pkgin".to_string(), vec!["list".to_string()]))
    } else {
        Ok(("pkg_info".to_string(), vec![name.to_string()]))
    }
}

// ---------- macOS ----------

#[cfg(target_os = "macos")]
fn check_package(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, stdout, _)) = exec("brew", &["list", "--versions", name]) {
        if !stdout.trim().is_empty() {
            state.installed = true;
            // Output: "nginx 1.24.0"
            if let Some(ver) = stdout.split_whitespace().nth(1) {
                state.installed_version = Some(ver.to_string());
            }
        }
    }
    if let Ok((0, stdout, _)) = exec("brew", &["outdated", "--json"]) {
        if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            for entry in &parsed {
                if entry.get("name").and_then(|n| n.as_str()) == Some(name) {
                    state.upgradable = true;
                    if let Some(v) = entry.get("installed_versions").and_then(|v| v.as_str()).or_else(|| {
                        entry.get("installed_versions").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_str())
                    }) {
                        state.installed_version = Some(v.to_string());
                    }
                    break;
                }
            }
        }
    }
    state
}

#[cfg(target_os = "macos")]
fn list_upgradable() -> Vec<String> {
    if let Ok((0, stdout, _)) = exec("brew", &["outdated", "--json"]) {
        if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
            return parsed.iter().filter_map(|e| e.get("name").and_then(|n| n.as_str()).map(|n| n.to_string())).collect();
        }
    }
    Vec::new()
}

#[cfg(target_os = "macos")]
fn get_check_command(name: &str) -> Result<(String, Vec<String>), String> {
    Ok(("brew".to_string(), vec!["list".to_string(), "--versions".to_string(), name.to_string()]))
}

// ---------- Linux ----------

#[cfg(target_os = "linux")]
fn check_package(name: &str) -> PkgState {
    let bin = detect_linux_pkg_manager().unwrap_or("apt-get");
    match bin {
        "apt-get" | "apt" => check_apt(name),
        "dnf" | "yum" => check_dnf(name),
        "zypper" => check_zypper(name),
        "pacman" => check_pacman(name),
        "apk" => check_apk(name),
        _ => PkgState::default(),
    }
}

#[cfg(target_os = "linux")]
fn list_upgradable() -> Vec<String> {
    let bin = detect_linux_pkg_manager().unwrap_or("apt-get");
    match bin {
        "apt-get" => upgradable_apt(),
        "dnf" | "yum" => upgradable_dnf(),
        "zypper" => upgradable_zypper(),
        "pacman" => upgradable_pacman(),
        "apk" => upgradable_apk(),
        _ => Vec::new(),
    }
}

#[cfg(target_os = "linux")]
fn check_apt(name: &str) -> PkgState {
    let mut state = PkgState::default();
    // dpkg -s: check installed + version
    if let Ok((0, stdout, _)) = exec("dpkg-query", &["-W", "-f=${Version}", name])
        && !stdout.trim().is_empty() {
            state.installed = true;
            state.installed_version = Some(stdout.trim().to_string());
        }
    // apt-cache policy: get candidate version
    if let Ok((0, stdout, _)) = exec("apt-cache", &["policy", name]) {
        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("  Candidate: ") {
                state.available_version = Some(v.trim().to_string());
                break;
            }
        }
    }
    // Upgradable?
    if state.installed
        && let Some(ref inst) = state.installed_version
            && let Some(ref cand) = state.available_version
        {
            state.upgradable = inst != cand;
        }
    state
}

#[cfg(target_os = "linux")]
fn upgradable_apt() -> Vec<String> {
    if let Ok((0, stdout, _)) = exec("apt-get", &["-s", "upgrade"]) {
        stdout
            .lines()
            .filter(|l| l.starts_with("Inst "))
            .filter_map(|l| l.split_whitespace().nth(1).map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn check_dnf(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, stdout, _)) = exec("dnf", &["repoquery", "--installed", "--qf=%{version}-%{release}", name]) {
        let v = stdout.trim().to_string();
        if !v.is_empty() {
            state.installed = true;
            state.installed_version = Some(v);
        }
    }
    if state.installed
        && let Ok((0, stdout, _)) = exec("dnf", &["check-update", name]) {
            // non-zero exit if updates exist, stdout has package info
            if !stdout.trim().is_empty() && !stdout.contains("Last metadata") {
                state.upgradable = true;
            }
        }
    state
}

#[cfg(target_os = "linux")]
fn upgradable_dnf() -> Vec<String> {
    if let Ok((_, stdout, _)) = exec("dnf", &["check-update"]) {
        stdout
            .lines()
            .filter(|l| !l.starts_with("Last ") && !l.is_empty())
            .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn check_zypper(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, stdout, _)) = exec("zypper", &["info", name]) {
        for line in stdout.lines() {
            if line.starts_with("Installed") && line.contains("Yes") {
                state.installed = true;
            }
            if let Some(v) = line.strip_prefix("Version        : ")
                && state.installed {
                    state.installed_version = Some(v.trim().to_string());
                }
        }
    }
    if state.installed
        && let Ok((_, stdout, _)) = exec("zypper", &["list-updates", "-t", "package"]) {
            state.upgradable = stdout.lines().any(|l| l.contains(name));
        }
    state
}

#[cfg(target_os = "linux")]
fn upgradable_zypper() -> Vec<String> {
    if let Ok((_, stdout, _)) = exec("zypper", &["list-updates", "-t", "package"]) {
        stdout
            .lines()
            .filter(|l| !l.starts_with("Loading ") && !l.starts_with('-') && !l.starts_with("S |"))
            .filter_map(|l| l.split('|').nth(1).map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty() && !s.contains("Name"))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn check_pacman(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, stdout, _)) = exec("pacman", &["-Qi", name]) {
        state.installed = true;
        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("Version         : ") {
                state.installed_version = Some(v.trim().to_string());
                break;
            }
        }
    }
    if let Ok((0, stdout, _)) = exec("pacman", &["-Si", name]) {
        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("Version         : ") {
                state.available_version = Some(v.trim().to_string());
                break;
            }
        }
    }
    if state.installed && state.installed_version != state.available_version {
        state.upgradable = true;
    }
    state
}

#[cfg(target_os = "linux")]
fn upgradable_pacman() -> Vec<String> {
    if let Ok((0, stdout, _)) = exec("pacman", &["-Qu"]) {
        stdout
            .lines()
            .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn check_apk(name: &str) -> PkgState {
    let mut state = PkgState::default();
    if let Ok((0, ..)) = exec("apk", &["info", "-e", name]) {
        state.installed = true;
        if let Ok((0, stdout, _)) = exec("apk", &["info", name])
            && let Some(line) = stdout.lines().next()
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                state.installed_version = Some(parts[0].to_string() + "-" + parts.get(1).unwrap_or(&""));
            }
        }
    }
    if state.installed
        && let Ok((_, stdout, _)) = exec("apk", &["list", "-u"]) {
            state.upgradable = stdout.lines().any(|l| {
                let pkg = l.split_whitespace().next().unwrap_or("");
                pkg == name || pkg.starts_with(&format!("{}-", name))
            });
        }
    state
}

#[cfg(target_os = "linux")]
fn upgradable_apk() -> Vec<String> {
    if let Ok((_, stdout, _)) = exec("apk", &["list", "-u"]) {
        stdout.lines().filter_map(|l| l.split_whitespace().next().map(|s| s.to_string())).collect()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn get_check_command(name: &str) -> Result<(String, Vec<String>), String> {
    let bin = detect_linux_pkg_manager().unwrap_or("apt-get");
    match bin {
        "apt-get" => Ok(("dpkg-query".to_string(), vec!["-W".to_string(), name.to_string()])),
        "dnf" | "yum" => Ok(("dnf".to_string(), vec!["repoquery".to_string(), "--installed".to_string(), name.to_string()])),
        "zypper" => Ok(("zypper".to_string(), vec!["info".to_string(), name.to_string()])),
        "pacman" => Ok(("pacman".to_string(), vec!["-Qi".to_string(), name.to_string()])),
        "apk" => Ok(("apk".to_string(), vec!["info".to_string(), name.to_string()])),
        _ => Err("No check command available".to_string()),
    }
}

// ===================================================================
// Unsupported OS fallback (check / upgradable)
// ===================================================================

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "macos"
)))]
fn check_package(_name: &str) -> PkgState {
    PkgState::default()
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "macos"
)))]
fn list_upgradable() -> Vec<String> {
    Vec::new()
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "macos"
)))]
fn get_check_command(_name: &str) -> Result<(String, Vec<String>), String> {
    Err("Unsupported operating system".to_string())
}

// ===================================================================
// Shared helper
// ===================================================================

fn exec(cmd: &str, args: &[&str]) -> Result<(i32, String, String), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(1);

    Ok((code, stdout, stderr))
}
