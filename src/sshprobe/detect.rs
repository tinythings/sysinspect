//! Reusable SSH platform detection for remote onboarding.

use libcommon::SysinspectError;
use std::{collections::{BTreeSet, HashMap}, env, process::Command, sync::Arc};

const SSH_TIMEOUT_SECS: u64 = 5;
const SYSTEM_DIRS: [&str; 5] = ["/usr/bin", "/etc", "/var/run", "/var/tmp", "/usr/share"];
const FALLBACK_DIRS: [&str; 4] = ["/tmp", "/var/tmp", "/dev/shm", "."];

/// Remote platform family inferred from `uname -s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlatformFamily {
    Linux,
    FreeBsd,
    NetBsd,
    OpenBsd,
    Qnx,
    Unknown,
}

/// Remote CPU architecture inferred from `uname -m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CpuArch {
    X86_64,
    Aarch64,
    Arm,
    X86,
    RiscV64,
    Ppc64Le,
    Unknown,
}

/// How probe data was collected on the remote host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecMode {
    Userland,
    Hybrid,
}

/// Effective privilege mode of the SSH login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivilegeMode {
    Root,
    Sudo,
    User,
}

/// Probed destination status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbePath {
    pub(crate) kind: ProbePathKind,
    pub(crate) requested: Option<String>,
    pub(crate) resolved: Option<String>,
    pub(crate) writable: bool,
}

/// Kind of destination check performed by the detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbePathKind {
    System,
    Custom,
}

/// Structured SSH probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeInfo {
    pub(crate) host: String,
    pub(crate) user: String,
    pub(crate) family: PlatformFamily,
    pub(crate) arch: CpuArch,
    pub(crate) exec_mode: ExecMode,
    pub(crate) privilege: PrivilegeMode,
    pub(crate) os_name: String,
    pub(crate) release: String,
    pub(crate) version: String,
    pub(crate) home: Option<String>,
    pub(crate) shell: Option<String>,
    pub(crate) tmp: Option<String>,
    pub(crate) has_sudo: bool,
    pub(crate) disk_free_bytes: Option<u64>,
    pub(crate) disk_free_path: Option<String>,
    pub(crate) destination: ProbePath,
    pub(crate) writable_paths: Vec<String>,
}

/// Reusable SSH-backed platform detector.
#[derive(Clone)]
pub(crate) struct SSHPlatformDetector {
    host: String,
    user: Option<String>,
    destination: Option<String>,
    check_writable: bool,
    runner: Arc<dyn ProbeRunner>,
}

impl SSHPlatformDetector {
    /// Create a detector for one remote host.
    pub(crate) fn new(host: impl Into<String>) -> Self {
        Self { host: host.into(), user: None, destination: None, check_writable: false, runner: Arc::new(SystemRunner) }
    }

    /// Set the SSH login user.
    pub(crate) fn set_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the desired remote destination root.
    pub(crate) fn set_destination(mut self, path: impl Into<String>) -> Self {
        self.destination = Some(path.into());
        self
    }

    /// Enable or disable writable fallback scanning.
    pub(crate) fn check_writable(mut self, enabled: bool) -> Self {
        self.check_writable = enabled;
        self
    }

    /// Probe the remote host and return normalised platform data.
    pub(crate) fn info(&self) -> Result<ProbeInfo, SysinspectError> {
        let user = self
            .user
            .clone()
            .or_else(current_user)
            .ok_or_else(|| SysinspectError::ConfigError(format!("No SSH user could be resolved for {}", self.host)))?;
        let mut raw = parse_kv(&self.runner.run(&self.host, &user, &userland_script())?);
        let mut exec_mode = ExecMode::Userland;
        if needs_proc_fallback(&raw) {
            merge_missing(&mut raw, parse_kv(&self.runner.run(&self.host, &user, &procfs_script())?));
            exec_mode = ExecMode::Hybrid;
        }
        let home = raw.get("home").cloned().filter(|v| !v.is_empty());
        let tmp = raw.get("tmp").cloned().filter(|v| !v.is_empty());
        let family = PlatformFamily::from_uname(raw.get("os").map(String::as_str).unwrap_or_default());
        let arch = CpuArch::from_uname(raw.get("arch").map(String::as_str).unwrap_or_default());
        if family == PlatformFamily::Unknown || arch == CpuArch::Unknown || tmp.is_none() {
            return Err(SysinspectError::MinionGeneralError(format!(
                "Unsupported target {}: probe could not determine platform={}, arch={}, tmp={}",
                self.host,
                raw.get("os").cloned().unwrap_or_else(|| "unknown".to_string()),
                raw.get("arch").cloned().unwrap_or_else(|| "unknown".to_string()),
                tmp.clone().unwrap_or_else(|| "missing".to_string())
            )));
        }
        let dest = self.destination_status(&user, home.as_deref())?;
        let (free_path, free_bytes) = self.disk_free_target(&user, &dest, home.as_deref(), tmp.as_deref())?;
        let writable_paths =
            if self.check_writable && !dest.writable { self.scan_writable_paths(&user, home.as_deref(), tmp.as_deref())? } else { Vec::new() };

        Ok(ProbeInfo {
            host: self.host.clone(),
            user,
            family,
            arch,
            exec_mode,
            privilege: privilege_mode(raw.get("sudo").is_some_and(|v| v == "yes"), raw.get("uid").map(String::as_str)),
            os_name: raw.get("os").cloned().unwrap_or_else(|| "unknown".to_string()),
            release: raw.get("release").cloned().unwrap_or_else(|| "unknown".to_string()),
            version: raw.get("version").cloned().unwrap_or_else(|| "unknown".to_string()),
            home,
            shell: raw.get("shell").cloned().filter(|v| !v.is_empty()),
            tmp,
            has_sudo: raw.get("sudo").is_some_and(|v| v == "yes"),
            disk_free_bytes: free_bytes,
            disk_free_path: free_path,
            destination: dest,
            writable_paths,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_runner(mut self, runner: Arc<dyn ProbeRunner>) -> Self {
        self.runner = runner;
        self
    }

    fn destination_status(&self, user: &str, home: Option<&str>) -> Result<ProbePath, SysinspectError> {
        match self.destination.as_deref() {
            Some(path) => {
                let resolved = resolve_remote_path(home, path);
                let writable = self.runner.run(&self.host, user, &writable_script(resolved.as_deref().unwrap_or(path)))?.trim().eq("yes");
                Ok(ProbePath { kind: ProbePathKind::Custom, requested: Some(path.to_string()), resolved, writable })
            }
            None => Ok(ProbePath {
                kind: ProbePathKind::System,
                requested: None,
                resolved: None,
                writable: self.runner.run(&self.host, user, &system_writable_script())?.trim().eq("yes"),
            }),
        }
    }

    fn disk_free_target(
        &self, user: &str, dest: &ProbePath, home: Option<&str>, tmp: Option<&str>,
    ) -> Result<(Option<String>, Option<u64>), SysinspectError> {
        let path = dest.resolved.clone().or_else(|| tmp.map(str::to_string)).or_else(|| home.map(str::to_string));
        let Some(path) = path else {
            return Ok((None, None));
        };
        let free = self.runner.run(&self.host, user, &disk_free_script(&path))?;
        Ok((Some(path), blocks_to_bytes(free.trim())))
    }

    fn scan_writable_paths(&self, user: &str, home: Option<&str>, tmp: Option<&str>) -> Result<Vec<String>, SysinspectError> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for path in
            home.into_iter().map(str::to_string).chain(tmp.into_iter().map(str::to_string)).chain(FALLBACK_DIRS.into_iter().map(str::to_string))
        {
            if !seen.insert(path.clone()) {
                continue;
            }
            if self.runner.run(&self.host, user, &writable_script(&path))?.trim().eq("yes") {
                out.push(path);
            }
        }
        Ok(out)
    }
}

impl PlatformFamily {
    fn from_uname(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "linux" => Self::Linux,
            "freebsd" => Self::FreeBsd,
            "netbsd" => Self::NetBsd,
            "openbsd" => Self::OpenBsd,
            "qnx" | "nto" => Self::Qnx,
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::FreeBsd => "FreeBSD",
            Self::NetBsd => "NetBSD",
            Self::OpenBsd => "OpenBSD",
            Self::Qnx => "QNX",
            Self::Unknown => "Unknown",
        }
    }
}

impl CpuArch {
    fn from_uname(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" => Self::X86_64,
            "aarch64" | "arm64" => Self::Aarch64,
            "arm" | "armv7l" | "armv6l" => Self::Arm,
            "i386" | "i486" | "i586" | "i686" => Self::X86,
            "riscv64" => Self::RiscV64,
            "ppc64le" => Self::Ppc64Le,
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "arm64",
            Self::Arm => "arm",
            Self::X86 => "x86",
            Self::RiscV64 => "riscv64",
            Self::Ppc64Le => "ppc64le",
            Self::Unknown => "unknown",
        }
    }
}

impl ExecMode {
    fn label(self) -> &'static str {
        match self {
            Self::Userland => "userland",
            Self::Hybrid => "hybrid",
        }
    }
}

impl PrivilegeMode {
    fn label(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Sudo => "sudo",
            Self::User => "user",
        }
    }
}

impl ProbeInfo {
    /// Return a compact operator-facing summary.
    pub(crate) fn summary(&self) -> String {
        format!(
            "{}/{} tmp={} priv={} exec={}",
            self.family.label(),
            self.arch.label(),
            self.tmp.as_deref().unwrap_or("?"),
            self.privilege.label(),
            self.exec_mode.label()
        )
    }
}

pub(crate) trait ProbeRunner: Send + Sync {
    fn run(&self, host: &str, user: &str, script: &str) -> Result<String, SysinspectError>;
}

struct SystemRunner;

impl ProbeRunner for SystemRunner {
    fn run(&self, host: &str, user: &str, script: &str) -> Result<String, SysinspectError> {
        let out = Command::new("ssh")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg(format!("ConnectTimeout={SSH_TIMEOUT_SECS}"))
            .arg(format!("{user}@{host}"))
            .arg(format!("sh -lc {}", shell_quote(script)))
            .output()?;
        if out.status.success() {
            return String::from_utf8(out.stdout)
                .map_err(|e| SysinspectError::DeserializationError(format!("SSH probe output is not valid UTF-8: {e}")));
        }
        Err(SysinspectError::MinionGeneralError(format!("SSH probe failed on {}: {}", host, String::from_utf8_lossy(&out.stderr).trim())))
    }
}

fn userland_script() -> String {
    [
        "os=$(uname -s 2>/dev/null || printf unknown)",
        "arch=$(uname -m 2>/dev/null || printf unknown)",
        "release=$(uname -r 2>/dev/null || printf unknown)",
        "version=$(uname -v 2>/dev/null || printf unknown)",
        "uid=$(id -u 2>/dev/null || printf unknown)",
        "home=${HOME:-}",
        "shell=${SHELL:-}",
        "sudo=no",
        "command -v sudo >/dev/null 2>&1 && sudo=yes",
        "tmp=",
        "for d in \"${TMPDIR:-}\" /tmp /var/tmp /dev/shm .; do [ -n \"$d\" ] || continue; [ -d \"$d\" ] || continue; tmp=$d; break; done",
        "printf 'os=%s\\narch=%s\\nrelease=%s\\nversion=%s\\nuid=%s\\nhome=%s\\nshell=%s\\ntmp=%s\\nsudo=%s\\n' \"$os\" \"$arch\" \"$release\" \"$version\" \"$uid\" \"$home\" \"$shell\" \"$tmp\" \"$sudo\"",
    ]
    .join("; ")
}

fn procfs_script() -> String {
    [
        "os=unknown",
        "release=unknown",
        "version=unknown",
        "arch=unknown",
        "uid=unknown",
        "[ -r /proc/sys/kernel/ostype ] && IFS= read -r os </proc/sys/kernel/ostype",
        "[ -r /proc/sys/kernel/osrelease ] && IFS= read -r release </proc/sys/kernel/osrelease",
        "[ -r /proc/sys/kernel/version ] && IFS= read -r version </proc/sys/kernel/version",
        "if [ -r /proc/cpuinfo ]; then while IFS= read -r line; do case \"$line\" in *AArch64*|*ARMv8*|*arm64*) arch=aarch64; break ;; *x86_64*|*amd64*) arch=x86_64; break ;; *riscv64*) arch=riscv64; break ;; *ppc64le*) arch=ppc64le; break ;; *ARMv7*|*ARMv6*|*armv7l*|*armv6l*|*arm*) arch=arm; break ;; *i386*|*i486*|*i586*|*i686*) arch=i386; break ;; esac; done </proc/cpuinfo; fi",
        "tmp=",
        "for d in /tmp /var/tmp /dev/shm .; do [ -d \"$d\" ] || continue; tmp=$d; break; done",
        "printf 'os=%s\\narch=%s\\nrelease=%s\\nversion=%s\\nuid=%s\\ntmp=%s\\n' \"$os\" \"$arch\" \"$release\" \"$version\" \"$uid\" \"$tmp\"",
    ]
    .join("; ")
}

fn writable_script(path: &str) -> String {
    format!(
        "p={}; c=\"$p\"; while [ ! -e \"$c\" ] && [ \"$c\" != \"/\" ]; do case \"$c\" in */*) c=${{c%/*}}; [ -n \"$c\" ] || c=/ ;; *) c=. ;; esac; done; [ -w \"$c\" ] && printf yes || printf no",
        shell_quote(path)
    )
}

fn system_writable_script() -> String {
    let checks = SYSTEM_DIRS.iter().map(|path| format!("[ -d {0} ] && [ -w {0} ]", shell_quote(path))).collect::<Vec<_>>().join(" && ");
    format!("{checks} && printf yes || printf no")
}

fn disk_free_script(path: &str) -> String {
    format!("df -Pk {} 2>/dev/null | {{ IFS= read -r _; IFS=' ' read -r _ _ _ avail _ || exit 0; printf '%s\\n' \"$avail\"; }}", shell_quote(path))
}

fn resolve_remote_path(home: Option<&str>, path: &str) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }
    if path.starts_with('/') {
        return Some(path.to_string());
    }
    home.map(|home| {
        let home = home.trim_end_matches('/');
        if home.is_empty() { path.to_string() } else { format!("{home}/{path}") }
    })
}

fn blocks_to_bytes(value: &str) -> Option<u64> {
    value.parse::<u64>().ok().and_then(|v| v.checked_mul(1024))
}

fn current_user() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"].into_iter().find_map(|key| env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn parse_kv(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in raw.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        out.insert(k.to_string(), v.to_string());
    }
    out
}

fn merge_missing(dst: &mut HashMap<String, String>, src: HashMap<String, String>) {
    for (k, v) in src {
        if v.is_empty() || v == "unknown" {
            continue;
        }
        match dst.get(&k) {
            Some(cur) if !cur.is_empty() && cur != "unknown" => {}
            _ => {
                dst.insert(k, v);
            }
        }
    }
}

fn needs_proc_fallback(raw: &HashMap<String, String>) -> bool {
    ["os", "arch", "tmp"].into_iter().any(|k| raw.get(k).is_none_or(|v| v.is_empty() || v == "unknown"))
}

fn privilege_mode(has_sudo: bool, uid: Option<&str>) -> PrivilegeMode {
    if uid == Some("0") {
        PrivilegeMode::Root
    } else if has_sudo {
        PrivilegeMode::Sudo
    } else {
        PrivilegeMode::User
    }
}
