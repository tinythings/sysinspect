//! Reusable SSH command execution and file upload layer.

use libcommon::SysinspectError;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::Arc,
};

const SSH_TIMEOUT_SECS: u64 = 5;

/// Remote SSH endpoint descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SSHEndpoint {
    pub(crate) host: String,
    pub(crate) user: String,
    pub(crate) port: Option<u16>,
}

/// Remote command request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteCommand {
    pub(crate) command: String,
    pub(crate) elevate: ElevationMode,
}

/// Requested privilege escalation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElevationMode {
    None,
    Sudo,
    Pfexec,
}

/// Upload backend choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UploadMethod {
    Scp,
    Stream,
}

/// Upload request with fallback methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UploadRequest {
    pub(crate) local: PathBuf,
    pub(crate) remote: String,
    pub(crate) methods: Vec<UploadMethod>,
}

/// Captured remote execution result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SSHResponse {
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Structured SSH session error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SSHFailure {
    pub(crate) op: &'static str,
    pub(crate) host: String,
    pub(crate) stderr: String,
}

/// Remote path resolver using probed home data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemotePathResolver {
    home: Option<String>,
}

/// Reusable SSH session object.
#[derive(Clone)]
pub(crate) struct SSHSession {
    ep: SSHEndpoint,
    run: Arc<dyn CommandRunner>,
}

pub(crate) trait CommandRunner: Send + Sync {
    fn run(&self, prog: &str, args: &[String], stdin: Option<Vec<u8>>) -> Result<SSHResponse, SysinspectError>;
}

struct SystemCommandRunner;

impl SSHEndpoint {
    /// Create one SSH endpoint.
    pub(crate) fn new(host: impl Into<String>, user: impl Into<String>) -> Self {
        Self { host: host.into(), user: user.into(), port: None }
    }

    /// Override the SSH port.
    pub(crate) fn set_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    fn login(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }
}

impl RemoteCommand {
    /// Create one remote command.
    pub(crate) fn new(command: impl Into<String>) -> Self {
        Self { command: command.into(), elevate: ElevationMode::None }
    }

    /// Request one privilege escalation mode.
    pub(crate) fn elevate(mut self, elevate: ElevationMode) -> Self {
        self.elevate = elevate;
        self
    }

    fn shell(&self) -> String {
        match self.elevate {
            ElevationMode::None => format!("sh -lc {}", shell_quote(&self.command)),
            ElevationMode::Sudo => format!("sudo -n sh -lc {}", shell_quote(&self.command)),
            ElevationMode::Pfexec => format!("pfexec sh -lc {}", shell_quote(&self.command)),
        }
    }
}

impl UploadRequest {
    /// Create one upload request.
    pub(crate) fn new(local: impl Into<PathBuf>, remote: impl Into<String>) -> Self {
        Self { local: local.into(), remote: remote.into(), methods: vec![UploadMethod::Scp, UploadMethod::Stream] }
    }

    /// Override the upload method order.
    pub(crate) fn methods(mut self, methods: Vec<UploadMethod>) -> Self {
        self.methods = methods;
        self
    }
}

impl RemotePathResolver {
    /// Create one resolver from an optional remote home.
    pub(crate) fn new(home: Option<String>) -> Self {
        Self { home }
    }

    /// Resolve one remote path with user-home semantics.
    pub(crate) fn resolve(&self, path: Option<&str>) -> Option<String> {
        match path.map(str::trim).filter(|v| !v.is_empty()) {
            Some(path) if path.starts_with('/') => Some(path.to_string()),
            Some(path) => self.home.as_deref().map(|home| format!("{}/{}", home.trim_end_matches('/'), path)),
            None => None,
        }
    }
}

impl SSHSession {
    /// Create one reusable SSH session.
    pub(crate) fn new(ep: SSHEndpoint) -> Self {
        Self { ep, run: Arc::new(SystemCommandRunner) }
    }

    #[cfg(test)]
    pub(crate) fn with_runner(mut self, run: Arc<dyn CommandRunner>) -> Self {
        self.run = run;
        self
    }

    /// Execute one remote command and capture stdout/stderr.
    pub(crate) fn exec(&self, cmd: &RemoteCommand) -> Result<SSHResponse, SysinspectError> {
        let mut args = ssh_base_args(&self.ep);
        args.push(self.ep.login());
        args.push(cmd.shell());
        let rsp = self.run.run("ssh", &args, None)?;
        if rsp.code == 0 {
            return Ok(rsp);
        }
        Err(ssh_fail("exec", &self.ep.host, &rsp.stderr))
    }

    /// Upload one local file to the remote host using the configured fallback order.
    pub(crate) fn upload(&self, req: &UploadRequest) -> Result<(), SysinspectError> {
        let mut last = None;
        for method in &req.methods {
            match method {
                UploadMethod::Scp => match self.upload_scp(req) {
                    Ok(()) => return Ok(()),
                    Err(err) => last = Some(err),
                },
                UploadMethod::Stream => match self.upload_stream(req) {
                    Ok(()) => return Ok(()),
                    Err(err) => last = Some(err),
                },
            }
        }
        Err(last.unwrap_or_else(|| ssh_fail("upload", &self.ep.host, "no upload method succeeded")))
    }

    fn upload_scp(&self, req: &UploadRequest) -> Result<(), SysinspectError> {
        let mut args = scp_base_args(&self.ep);
        args.push(req.local.display().to_string());
        args.push(format!("{}:{}", self.ep.login(), req.remote));
        let rsp = self.run.run("scp", &args, None)?;
        if rsp.code == 0 {
            return Ok(());
        }
        Err(ssh_fail("upload", &self.ep.host, &rsp.stderr))
    }

    fn upload_stream(&self, req: &UploadRequest) -> Result<(), SysinspectError> {
        let data = std::fs::read(&req.local)?;
        let parent = Path::new(&req.remote).parent().map(|p| p.display().to_string()).unwrap_or_else(|| ".".to_string());
        let cmd = format!("mkdir -p {} && cat > {}", shell_quote(&parent), shell_quote(&req.remote));
        let mut args = ssh_base_args(&self.ep);
        args.push(self.ep.login());
        args.push(format!("sh -lc {}", shell_quote(&cmd)));
        let rsp = self.run.run("ssh", &args, Some(data))?;
        if rsp.code == 0 {
            return Ok(());
        }
        Err(ssh_fail("upload", &self.ep.host, &rsp.stderr))
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, prog: &str, args: &[String], stdin: Option<Vec<u8>>) -> Result<SSHResponse, SysinspectError> {
        let mut cmd = Command::new(prog);
        cmd.args(args);
        if stdin.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }
        let mut child = cmd.spawn()?;
        if let Some(stdin) = stdin
            && let Some(mut pipe) = child.stdin.take()
        {
            use std::io::Write;
            pipe.write_all(&stdin)?;
        }
        let out = child.wait_with_output()?;
        Ok(into_rsp(out))
    }
}

fn ssh_base_args(ep: &SSHEndpoint) -> Vec<String> {
    let mut out = vec!["-o".to_string(), "BatchMode=yes".to_string(), "-o".to_string(), format!("ConnectTimeout={SSH_TIMEOUT_SECS}")];
    if let Some(port) = ep.port {
        out.push("-p".to_string());
        out.push(port.to_string());
    }
    out
}

fn scp_base_args(ep: &SSHEndpoint) -> Vec<String> {
    let mut out =
        vec!["-q".to_string(), "-o".to_string(), "BatchMode=yes".to_string(), "-o".to_string(), format!("ConnectTimeout={SSH_TIMEOUT_SECS}")];
    if let Some(port) = ep.port {
        out.push("-P".to_string());
        out.push(port.to_string());
    }
    out
}

fn into_rsp(out: Output) -> SSHResponse {
    SSHResponse {
        code: out.status.code().unwrap_or(255),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn ssh_fail(op: &'static str, host: &str, stderr: &str) -> SysinspectError {
    let err = SSHFailure { op, host: host.to_string(), stderr: stderr.trim().to_string() };
    SysinspectError::MinionGeneralError(format!("SSH {} failed on {}: {}", err.op, err.host, err.stderr))
}

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
