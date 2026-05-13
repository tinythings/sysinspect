use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::process::Command;

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut response = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");
    let dry_run = runtime::get_opt(rt, "dry-run");

    let op = get_operation(rt, &mut response);
    if op.is_empty() {
        return response;
    }

    match get_pkg_command(&op, &name) {
        Ok((cmd, args)) => {
            if dry_run {
                response.set_retcode(0);
                response.set_message(&format!("[dry-run] {} {}", cmd, args.join(" ")));
                return response;
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
                        response.add_warning(&stdout.trim());
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

    response
}

pub(crate) fn get_operation(rt: &ModRequest, response: &mut ModResponse) -> String {
    if runtime::get_opt(rt, "install") {
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
        response.set_message("No operation specified. Use one of: --install, --remove, --update, --upgrade, --search");
        String::new()
    }
}

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
