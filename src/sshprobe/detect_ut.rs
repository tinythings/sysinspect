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
        Ok("os=Linux\narch=x86_64\nrelease=6.8\nversion=#1\nhome=/home/hans\nshell=/bin/sh\ntmp=/tmp\nsudo=yes\n".to_string()),
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
    assert_eq!(info.disk_free_bytes, Some(2048 * 1024));
    assert_eq!(info.disk_free_path.as_deref(), Some("/home/hans/foo"));
    assert_eq!(info.destination.kind, ProbePathKind::Custom);
    assert_eq!(info.destination.requested.as_deref(), Some("foo"));
    assert_eq!(info.destination.resolved.as_deref(), Some("/home/hans/foo"));
    assert!(!info.destination.writable);
    assert_eq!(info.writable_paths, &["/home/hans".to_string(), "/tmp".to_string(), ".".to_string()]);
}

#[test]
fn probes_system_destination() {
    let runner = FakeRunner::with(vec![
        Ok("os=FreeBSD\narch=amd64\nrelease=14.0\nversion=GENERIC\nhome=/usr/home/hans\nshell=/bin/sh\ntmp=/var/tmp\nsudo=no\n".to_string()),
        Ok("yes".to_string()),
        Ok("4096\n".to_string()),
    ]);
    let info = SSHPlatformDetector::new("bsd-box").set_user("hans").with_runner(runner).info().unwrap();

    assert_eq!(info.family, PlatformFamily::FreeBsd);
    assert_eq!(info.arch, CpuArch::X86_64);
    assert_eq!(info.os_name, "FreeBSD");
    assert_eq!(info.destination.kind, ProbePathKind::System);
    assert!(info.destination.writable);
    assert_eq!(info.disk_free_bytes, Some(4096 * 1024));
    assert!(info.writable_paths.is_empty());
}

#[test]
fn maps_qnx_and_unknown_arch() {
    let runner = FakeRunner::with(vec![
        Ok("os=QNX\narch=weirdcpu\nrelease=7\nversion=1\nhome=/home/qnx\nshell=/bin/sh\ntmp=/tmp\nsudo=no\n".to_string()),
        Ok("yes".to_string()),
        Ok("1024\n".to_string()),
    ]);
    let info = SSHPlatformDetector::new("qnx-box").set_user("root").with_runner(runner).info().unwrap();

    assert_eq!(info.family, PlatformFamily::Qnx);
    assert_eq!(info.arch, CpuArch::Unknown);
}
