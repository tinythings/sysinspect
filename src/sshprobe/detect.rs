//! Reusable SSH platform detection for remote onboarding.

use crate::sshprobe::transport::{RemoteCommand, SSHEndpoint, SSHSession, shell_quote};
use libcommon::SysinspectError;
use std::{
    collections::{BTreeSet, HashMap},
    env,
    sync::Arc,
};

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
    Home,
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
            .or_else(|| {
                ["USER", "LOGNAME", "USERNAME"]
                    .into_iter()
                    .find_map(|key| env::var(key).ok().filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string()))
            })
            .ok_or_else(|| SysinspectError::ConfigError(format!("No SSH user could be resolved for {}", self.host)))?;
        let mut raw = parse_kv(&self.runner.run(&self.host, &user, &userland_script())?);
        let mut exec_mode = ExecMode::Userland;
        if ["os", "arch", "tmp"].into_iter().any(|k| raw.get(k).is_none_or(|v| v.is_empty() || v == "unknown")) {
            merge_missing(&mut raw, parse_kv(&self.runner.run(&self.host, &user, &procfs_script())?));
            exec_mode = ExecMode::Hybrid;
        }
        if PlatformFamily::from_uname(raw.get("os").map(String::as_str).unwrap_or_default()) == PlatformFamily::Unknown
            || CpuArch::from_uname(raw.get("arch").map(String::as_str).unwrap_or_default()) == CpuArch::Unknown
            || raw.get("tmp").is_none_or(|v| v.is_empty())
        {
            return Err(SysinspectError::MinionGeneralError(format!(
                "Unsupported target {}: probe could not determine platform={}, arch={}, tmp={}",
                self.host,
                raw.get("os").cloned().unwrap_or_else(|| "unknown".to_string()),
                raw.get("arch").cloned().unwrap_or_else(|| "unknown".to_string()),
                raw.get("tmp").cloned().filter(|v| !v.is_empty()).unwrap_or_else(|| "missing".to_string())
            )));
        }
        let home = raw.get("home").cloned().filter(|v| !v.is_empty());
        let tmp = raw.get("tmp").cloned().filter(|v| !v.is_empty());
        let destination = match self.destination.as_deref() {
            Some(path) => ProbePath {
                kind: ProbePathKind::Custom,
                requested: Some(path.to_string()),
                resolved: resolve_remote_path(home.as_deref(), path),
                writable: self
                    .runner
                    .run(&self.host, &user, &writable_script(resolve_remote_path(home.as_deref(), path).as_deref().unwrap_or(path)))?
                    .trim()
                    .eq("yes"),
            },
            None => ProbePath {
                kind: ProbePathKind::Home,
                requested: None,
                resolved: resolve_remote_path(home.as_deref(), "sysinspect").or_else(|| Some("sysinspect".to_string())),
                writable: self
                    .runner
                    .run(&self.host, &user, &writable_script(resolve_remote_path(home.as_deref(), "sysinspect").as_deref().unwrap_or("sysinspect")))?
                    .trim()
                    .eq("yes"),
            },
        };
        let (disk_free_path, disk_free_bytes) =
            match destination.resolved.clone().or_else(|| tmp.as_deref().map(str::to_string)).or_else(|| home.as_deref().map(str::to_string)) {
                Some(path) => (
                    Some(path.clone()),
                    self.runner
                        .run(
                            &self.host,
                            &user,
                            &format!(
                                "df -Pk {} 2>/dev/null | {{ IFS= read -r _; IFS=' ' read -r _ _ _ avail _ || exit 0; printf '%s\\n' \"$avail\"; }}",
                                shell_quote(&path)
                            ),
                        )?
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .and_then(|v| v.checked_mul(1024)),
                ),
                None => (None, None),
            };
        let writable_paths = if self.check_writable && !destination.writable {
            let mut seen = BTreeSet::new();
            let mut out = Vec::new();
            for path in home.iter().cloned().chain(tmp.iter().cloned()).chain(FALLBACK_DIRS.into_iter().map(str::to_string)) {
                if seen.insert(path.clone()) && self.runner.run(&self.host, &user, &writable_script(&path))?.trim().eq("yes") {
                    out.push(path);
                }
            }
            out
        } else {
            Vec::new()
        };

        Ok(ProbeInfo {
            host: self.host.clone(),
            user,
            family: PlatformFamily::from_uname(raw.get("os").map(String::as_str).unwrap_or_default()),
            arch: CpuArch::from_uname(raw.get("arch").map(String::as_str).unwrap_or_default()),
            exec_mode,
            privilege: if raw.get("uid").map(String::as_str) == Some("0") {
                PrivilegeMode::Root
            } else if raw.get("sudo").is_some_and(|v| v == "yes") {
                PrivilegeMode::Sudo
            } else {
                PrivilegeMode::User
            },
            os_name: raw.get("os").cloned().unwrap_or_else(|| "unknown".to_string()),
            release: raw.get("release").cloned().unwrap_or_else(|| "unknown".to_string()),
            version: raw.get("version").cloned().unwrap_or_else(|| "unknown".to_string()),
            home,
            shell: raw.get("shell").cloned().filter(|v| !v.is_empty()),
            tmp,
            has_sudo: raw.get("sudo").is_some_and(|v| v == "yes"),
            disk_free_path,
            disk_free_bytes,
            writable_paths,
            destination,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_runner(mut self, runner: Arc<dyn ProbeRunner>) -> Self {
        self.runner = runner;
        self
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
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Userland => "userland",
            Self::Hybrid => "hybrid",
        }
    }
}

impl PrivilegeMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Sudo => "sudo",
            Self::User => "user",
        }
    }
}

impl ProbeInfo {
    pub(crate) fn os_arch(&self) -> String {
        format!("{}/{}", self.family.label(), self.arch.label())
    }

    /// Return a compact operator-facing summary.
    pub(crate) fn summary(&self) -> String {
        format!("{} tmp={} priv={} exec={}", self.os_arch(), self.tmp.as_deref().unwrap_or("?"), self.privilege.label(), self.exec_mode.label())
    }
}

pub(crate) trait ProbeRunner: Send + Sync {
    fn run(&self, host: &str, user: &str, script: &str) -> Result<String, SysinspectError>;
}

struct SystemRunner;

impl ProbeRunner for SystemRunner {
    fn run(&self, host: &str, user: &str, script: &str) -> Result<String, SysinspectError> {
        Ok(SSHSession::new(SSHEndpoint::new(host, user)).exec(&RemoteCommand::new(script))?.stdout)
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

fn resolve_remote_path(home: Option<&str>, path: &str) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }
    if path.starts_with('/') {
        return Some(path.to_string());
    }
    home.map(|home| match home.trim_end_matches('/') {
        "" => path.to_string(),
        home => format!("{home}/{path}"),
    })
}

fn parse_kv(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::<String, String>::new();
    for line in raw.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match out.get(k) {
            Some(cur) if !cur.is_empty() && cur != "unknown" => {}
            _ => {
                out.insert(k.to_string(), v.to_string());
            }
        }
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
