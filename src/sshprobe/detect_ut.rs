use super::detect::*;
use libcommon::SysinspectError;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct FakeRunner {
    out: Mutex<VecDeque<Result<String, SysinspectError>>>,
}

impl FakeRunner {
    fn with(out: Vec<Result<String, SysinspectError>>) -> Arc<Self> {
        Arc::new(Self { out: Mutex::new(out.into()) })
    }
}

impl ProbeRunner for FakeRunner {
    fn run(&self, _host: &str, _user: &str, _script: &str) -> Result<String, SysinspectError> {
        self.out.lock().unwrap().pop_front().unwrap_or_else(|| Err(SysinspectError::MinionGeneralError("missing fake probe output".to_string())))
    }
}

#[test]
fn probes_custom_destination_and_fallbacks() {
    let runner = FakeRunner::with(vec![
        Ok("os=Linux\narch=x86_64\nrelease=6.8\nversion=#1\nuid=1000\nhome=/home/hans\nshell=/bin/sh\ntmp=/tmp\nsudo=yes\n".to_string()),
        Ok("no".to_string()),
        Ok("2048\n".to_string()),
        Ok("yes".to_string()),
        Ok("yes".to_string()),
        Ok("no".to_string()),
        Ok("no".to_string()),
        Ok("yes".to_string()),
        Ok("yes".to_string()),
    ]);
    let info =
        SSHPlatformDetector::new("somehost.com").set_user("hans").set_destination("foo").check_writable(true).with_runner(runner).info().unwrap();

    assert_eq!(info.family, PlatformFamily::Linux);
    assert_eq!(info.arch, CpuArch::X86_64);
    assert_eq!(info.exec_mode, ExecMode::Userland);
    assert_eq!(info.privilege, PrivilegeMode::Sudo);
    assert_eq!(info.disk_free_bytes, Some(2048 * 1024));
    assert_eq!(info.disk_free_path.as_deref(), Some("/home/hans/foo"));
    assert_eq!(info.destination.kind, ProbePathKind::Custom);
    assert_eq!(info.destination.requested.as_deref(), Some("foo"));
    assert_eq!(info.destination.resolved.as_deref(), Some("/home/hans/foo"));
    assert!(!info.destination.writable);
    assert_eq!(info.writable_paths, &["/home/hans".to_string(), "/tmp".to_string(), ".".to_string()]);
    assert_eq!(info.summary(), "Linux/x86_64 tmp=/tmp priv=sudo exec=userland");
}

#[test]
fn probes_system_destination() {
    let runner = FakeRunner::with(vec![
        Ok("os=FreeBSD\narch=amd64\nrelease=14.0\nversion=GENERIC\nuid=0\nhome=/usr/home/hans\nshell=/bin/sh\ntmp=/var/tmp\nsudo=no\n".to_string()),
        Ok("yes".to_string()),
        Ok("4096\n".to_string()),
    ]);
    let info = SSHPlatformDetector::new("bsd-box").set_user("hans").with_runner(runner).info().unwrap();

    assert_eq!(info.family, PlatformFamily::FreeBsd);
    assert_eq!(info.arch, CpuArch::X86_64);
    assert_eq!(info.exec_mode, ExecMode::Userland);
    assert_eq!(info.privilege, PrivilegeMode::Root);
    assert_eq!(info.os_name, "FreeBSD");
    assert_eq!(info.destination.kind, ProbePathKind::System);
    assert!(info.destination.writable);
    assert_eq!(info.disk_free_bytes, Some(4096 * 1024));
    assert!(info.writable_paths.is_empty());
}

#[test]
fn maps_qnx_and_unknown_arch() {
    let runner = FakeRunner::with(vec![Ok(
        "os=QNX\narch=weirdcpu\nrelease=7\nversion=1\nuid=0\nhome=/home/qnx\nshell=/bin/sh\ntmp=/tmp\nsudo=no\n".to_string()
    )]);
    let err = SSHPlatformDetector::new("qnx-box").set_user("root").with_runner(runner).info().unwrap_err();

    assert!(err.to_string().contains("Unsupported target"));
}

#[test]
fn falls_back_to_procfs_when_userland_is_missing() {
    let runner = FakeRunner::with(vec![
        Ok("os=unknown\narch=unknown\nrelease=unknown\nversion=unknown\nuid=1000\nhome=/home/hans\nshell=/bin/sh\ntmp=\nsudo=no\n".to_string()),
        Ok("os=NetBSD\narch=amd64\nrelease=10\nversion=GENERIC\nuid=1000\ntmp=/tmp\n".to_string()),
        Ok("yes".to_string()),
        Ok("1024\n".to_string()),
    ]);
    let info = SSHPlatformDetector::new("nb-box").set_user("hans").with_runner(runner).info().unwrap();

    assert_eq!(info.family, PlatformFamily::NetBsd);
    assert_eq!(info.arch, CpuArch::X86_64);
    assert_eq!(info.exec_mode, ExecMode::Hybrid);
    assert_eq!(info.privilege, PrivilegeMode::User);
    assert_eq!(info.tmp.as_deref(), Some("/tmp"));
}

#[test]
fn rejects_when_userland_and_procfs_fail() {
    let runner = FakeRunner::with(vec![
        Ok("os=unknown\narch=unknown\nrelease=unknown\nversion=unknown\nuid=1000\nhome=/home/hans\nshell=/bin/sh\ntmp=\nsudo=no\n".to_string()),
        Ok("os=unknown\narch=unknown\nrelease=unknown\nversion=unknown\nuid=1000\ntmp=\n".to_string()),
    ]);
    let err = SSHPlatformDetector::new("broken-box").set_user("hans").with_runner(runner).info().unwrap_err();

    assert!(err.to_string().contains("Unsupported target"));
}
