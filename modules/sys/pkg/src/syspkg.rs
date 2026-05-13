use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::process::Command;

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut response = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");

    let op = if runtime::get_opt(rt, "install") {
        "install"
    } else if runtime::get_opt(rt, "remove") {
        "remove"
    } else if runtime::get_opt(rt, "update") {
        "update"
    } else if runtime::get_opt(rt, "upgrade") {
        "upgrade"
    } else if runtime::get_opt(rt, "search") {
        "search"
    } else {
        response.set_retcode(1);
        response.set_message("No operation specified. Use one of: --install, --remove, --update, --upgrade, --search");
        return response;
    };

    let result = run_pkg_cmd(op, &name);
    match result {
        Ok((code, stdout, stderr)) => {
            response.set_retcode(code);
            if code == 0 {
                response.set_message(&format!("Package operation '{op}' completed"));
            } else {
                response.set_message(&format!("Package operation '{op}' failed: {stderr}"));
            }
            response.add_warning(&stdout);
        }
        Err(err) => {
            response.set_retcode(1);
            response.set_message(&err);
        }
    }

    response
}

#[cfg(target_os = "freebsd")]
fn run_pkg_cmd(op: &str, name: &str) -> Result<(i32, String, String), String> {
    let (cmd, args) = match op {
        "install" => ("pkg", vec!["install", "-y", name]),
        "remove" => ("pkg", vec!["delete", "-y", name]),
        "update" => ("pkg", vec!["update"]),
        "upgrade" => ("pkg", vec!["upgrade", "-y"]),
        "search" => ("pkg", vec!["search", name]),
        _ => return Err(format!("Unknown operation: {op}")),
    };

    exec(cmd, &args)
}

#[cfg(target_os = "openbsd")]
fn run_pkg_cmd(op: &str, name: &str) -> Result<(i32, String, String), String> {
    match op {
        "install" => exec("pkg_add", &[name]),
        "remove" => exec("pkg_delete", &[name]),
        "update" => Err("OpenBSD pkg_add has no repository update; packages are fetched directly".to_string()),
        "upgrade" => exec("pkg_add", &["-u", name]),
        "search" => exec("pkg_info", &["-Q", name]),
        _ => Err(format!("Unknown operation: {op}")),
    }
}

#[cfg(target_os = "netbsd")]
fn run_pkg_cmd(op: &str, name: &str) -> Result<(i32, String, String), String> {
    let has_pkgin = Command::new("pkgin")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_pkgin {
        match op {
            "install" => exec("pkgin", &["-y", "install", name]),
            "remove" => exec("pkgin", &["-y", "remove", name]),
            "update" => exec("pkgin", &["-y", "update"]),
            "upgrade" => exec("pkgin", &["-y", "upgrade"]),
            "search" => exec("pkgin", &["search", name]),
            _ => return Err(format!("Unknown operation: {op}")),
        }
    } else {
        match op {
            "install" => exec("pkg_add", &[name]),
            "remove" => exec("pkg_delete", &[name]),
            "update" => Err("NetBSD pkg_add has no repository update command".to_string()),
            "upgrade" => exec("pkg_add", &["-u", name]),
            "search" => Err("NetBSD: install pkgin for search support".to_string()),
            _ => Err(format!("Unknown operation: {op}")),
        }
    }
}

#[cfg(target_os = "macos")]
fn run_pkg_cmd(op: &str, name: &str) -> Result<(i32, String, String), String> {
    let has_brew = Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_brew {
        match op {
            "install" => exec("brew", &["install", name]),
            "remove" => exec("brew", &["uninstall", name]),
            "update" => exec("brew", &["update"]),
            "upgrade" => exec("brew", &["upgrade"]),
            "search" => exec("brew", &["search", name]),
            _ => Err(format!("Unknown operation: {op}")),
        }
    } else {
        Err("No supported package manager found on macOS (brew not detected)".to_string())
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_pkg_manager() -> Option<(&'static str, &'static str)> {
    let candidates: &[(&str, &str)] = &[
        ("apt-get", "deb"),
        ("dnf", "rpm"),
        ("yum", "rpm"),
        ("zypper", "rpm"),
        ("pacman", "arch"),
        ("apk", "apk"),
    ];

    candidates.iter().find_map(|(bin, family)| {
        Command::new(bin)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|_| (*bin, *family))
    })
}

#[cfg(target_os = "linux")]
fn run_pkg_cmd(op: &str, name: &str) -> Result<(i32, String, String), String> {
    let (bin, _family) = detect_linux_pkg_manager()
        .ok_or_else(|| "No supported package manager found (tried: apt-get, dnf, yum, zypper, pacman, apk)".to_string())?;

    let args: Vec<&str> = match (bin, op) {
        ("apt-get", "install") => vec!["install", "-y", name],
        ("apt-get", "remove") => vec!["remove", "-y", name],
        ("apt-get", "update") => vec!["update"],
        ("apt-get", "upgrade") => vec!["upgrade", "-y"],
        ("apt-get", "search") => return apt_search(name),

        ("dnf" | "yum", "install") => vec!["install", "-y", name],
        ("dnf" | "yum", "remove") => vec!["remove", "-y", name],
        ("dnf" | "yum", "update") => vec!["makecache"],
        ("dnf" | "yum", "upgrade") => vec!["upgrade", "-y"],
        ("dnf" | "yum", "search") => vec!["search", name],

        ("zypper", "install") => vec!["install", "-y", name],
        ("zypper", "remove") => vec!["remove", "-y", name],
        ("zypper", "update") => vec!["refresh"],
        ("zypper", "upgrade") => vec!["update", "-y"],
        ("zypper", "search") => vec!["search", name],

        ("pacman", "install") => vec!["-S", "--noconfirm", name],
        ("pacman", "remove") => vec!["-R", "--noconfirm", name],
        ("pacman", "update") => vec!["-Sy"],
        ("pacman", "upgrade") => vec!["-Su", "--noconfirm"],
        ("pacman", "search") => vec!["-Ss", name],

        ("apk", "install") => vec!["add", name],
        ("apk", "remove") => vec!["del", name],
        ("apk", "update") => vec!["update"],
        ("apk", "upgrade") => vec!["upgrade"],
        ("apk", "search") => vec!["search", name],

        _ => return Err(format!("Unsupported operation '{op}' for {bin}")),
    };

    exec(bin, &args)
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "macos"
)))]
fn run_pkg_cmd(_op: &str, _name: &str) -> Result<(i32, String, String), String> {
    Err("Unsupported operating system".to_string())
}

#[cfg(target_os = "linux")]
fn apt_search(name: &str) -> Result<(i32, String, String), String> {
    let output = Command::new("apt-cache")
        .args(["search", name])
        .output()
        .map_err(|e| format!("Failed to execute apt-cache: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(1);
    Ok((code, stdout, stderr))
}

fn exec(cmd: &str, args: &[&str]) -> Result<(i32, String, String), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {cmd}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(1);

    Ok((code, stdout, stderr))
}
