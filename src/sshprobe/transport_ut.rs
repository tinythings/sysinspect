use super::transport::*;
use libcommon::SysinspectError;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<(String, Vec<String>, Option<Vec<u8>>)>>,
    out: Mutex<VecDeque<Result<SSHResponse, SysinspectError>>>,
}

impl FakeRunner {
    fn with(out: Vec<Result<SSHResponse, SysinspectError>>) -> Arc<Self> {
        Arc::new(Self { calls: Mutex::new(Vec::new()), out: Mutex::new(out.into()) })
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, prog: &str, args: &[String], stdin: Option<Vec<u8>>) -> Result<SSHResponse, SysinspectError> {
        self.calls.lock().unwrap().push((prog.to_string(), args.to_vec(), stdin));
        self.out.lock().unwrap().pop_front().unwrap()
    }
}

#[test]
fn resolves_remote_paths_from_home() {
    let rs = RemotePathResolver::new(Some("/export/home/hans".to_string()));

    assert_eq!(rs.resolve(Some("tmp/foo")).as_deref(), Some("/export/home/hans/tmp/foo"));
    assert_eq!(rs.resolve(Some("/opt/foo")).as_deref(), Some("/opt/foo"));
}

#[test]
fn exec_wraps_remote_command() {
    let run = FakeRunner::with(vec![Ok(SSHResponse { code: 0, stdout: "ok".to_string(), stderr: String::new() })]);
    let rsp = SSHSession::new(SSHEndpoint::new("box", "hans"))
        .with_runner(run.clone())
        .exec(&RemoteCommand::new("uname -s").elevate(ElevationMode::Sudo))
        .unwrap();

    assert_eq!(rsp.stdout, "ok");
    assert_eq!(run.calls.lock().unwrap()[0].0, "ssh");
    assert!(run.calls.lock().unwrap()[0].1.iter().any(|v| v.contains("sudo -n sh -lc")));
}

#[test]
fn exec_supports_pfexec_for_solaris_like_targets() {
    let run = FakeRunner::with(vec![Ok(SSHResponse { code: 0, stdout: "ok".to_string(), stderr: String::new() })]);
    SSHSession::new(SSHEndpoint::new("box", "hans")).with_runner(run.clone()).exec(&RemoteCommand::new("id").elevate(ElevationMode::Pfexec)).unwrap();

    assert!(run.calls.lock().unwrap()[0].1.iter().any(|v| v.contains("pfexec sh -lc")));
}

#[test]
fn upload_falls_back_from_scp_to_stream() {
    let file = std::env::temp_dir().join(format!("sshprobe-upload-{}", std::process::id()));
    std::fs::write(&file, b"booya").unwrap();
    let run = FakeRunner::with(vec![
        Ok(SSHResponse { code: 1, stdout: String::new(), stderr: "scp failed".to_string() }),
        Ok(SSHResponse { code: 0, stdout: String::new(), stderr: String::new() }),
    ]);
    SSHSession::new(SSHEndpoint::new("box", "hans")).with_runner(run.clone()).upload(&UploadRequest::new(&file, "/tmp/foo")).unwrap();

    let calls = run.calls.lock().unwrap();
    assert_eq!(calls[0].0, "scp");
    assert_eq!(calls[1].0, "ssh");
    assert_eq!(calls[1].2.as_deref(), Some(b"booya".as_slice()));
    let _ = std::fs::remove_file(file);
}

#[test]
fn upload_honours_custom_method_order_and_port() {
    let file = std::env::temp_dir().join(format!("sshprobe-upload-order-{}", std::process::id()));
    std::fs::write(&file, b"booya").unwrap();
    let run = FakeRunner::with(vec![Ok(SSHResponse { code: 0, stdout: String::new(), stderr: String::new() })]);
    SSHSession::new(SSHEndpoint::new("box", "hans").set_port(2222))
        .with_runner(run.clone())
        .upload(&UploadRequest::new(&file, "/tmp/foo").methods(vec![UploadMethod::Stream]))
        .unwrap();

    let calls = run.calls.lock().unwrap();
    assert_eq!(calls[0].0, "ssh");
    assert!(calls[0].1.iter().any(|v| v == "2222"));
    let _ = std::fs::remove_file(file);
}

#[test]
fn upload_reports_failure_when_all_methods_fail() {
    let file = std::env::temp_dir().join(format!("sshprobe-upload-fail-{}", std::process::id()));
    std::fs::write(&file, b"booya").unwrap();
    let run = FakeRunner::with(vec![
        Ok(SSHResponse { code: 1, stdout: String::new(), stderr: "scp failed".to_string() }),
        Ok(SSHResponse { code: 1, stdout: String::new(), stderr: "stream failed".to_string() }),
    ]);
    let err =
        SSHSession::new(SSHEndpoint::new("box", "hans")).with_runner(run).upload(&UploadRequest::new(PathBuf::from(&file), "/tmp/foo")).unwrap_err();

    assert!(err.to_string().contains("SSH upload failed"));
    let _ = std::fs::remove_file(file);
}
