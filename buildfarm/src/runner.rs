use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    thread,
};

use portable_pty::{CommandBuilder, PtySize, PtySystem, native_pty_system};

use crate::model::{BuildTarget, BuildfarmConfig};

pub struct BuildPlan {
    jobs: Vec<BuildJob>,
}

impl BuildPlan {
    pub fn new(config: &BuildfarmConfig, entry: &str, root_dir: &Path, log_root: &Path, local_make: &str) -> Self {
        Self {
            jobs: config
                .targets()
                .iter()
                .map(|target| BuildJob::build(target, entry, root_dir, log_root, local_make))
                .collect(),
        }
    }

    pub fn jobs(&self) -> &[BuildJob] {
        &self.jobs
    }
}

pub struct BuildJob {
    target: BuildTarget,
    command: BuildCommand,
    log_path: PathBuf,
}

impl BuildJob {
    pub fn new(target: BuildTarget, command: BuildCommand, log_path: PathBuf) -> Self {
        Self { target, command, log_path }
    }

    pub fn build(target: &BuildTarget, entry: &str, root_dir: &Path, log_root: &Path, local_make: &str) -> Self {
        Self::new(
            target.clone(),
            BuildCommand::for_target(target, entry, root_dir, local_make),
            log_root.join(format!("{}.log", target.log_key())),
        )
    }

    pub fn target(&self) -> &BuildTarget {
        &self.target
    }

    pub fn command(&self) -> &BuildCommand {
        &self.command
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn run(&self) -> Result<JobResult, String> {
        fs::create_dir_all(self.log_path.parent().unwrap_or_else(|| Path::new(".")))
            .map_err(|err| format!("buildfarm: failed to create log directory: {err}"))?;
        self.command.run(&self.log_path).map(|status| JobResult::new(self.log_path.clone(), status))
    }
}

pub struct BuildCommand {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
}

impl BuildCommand {
    pub fn new(program: &str, args: Vec<String>, cwd: Option<PathBuf>) -> Self {
        Self {
            program: program.to_string(),
            args,
            cwd,
        }
    }

    pub fn for_target(target: &BuildTarget, entry: &str, root_dir: &Path, local_make: &str) -> Self {
        target
            .is_local()
            .then_some(Self::local(entry, root_dir, local_make))
            .unwrap_or_else(|| Self::remote(target, entry))
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    pub fn run(&self, log_path: &Path) -> Result<i32, String> {
        PtySession::new(self)?.run(log_path)
    }

    fn local(entry: &str, root_dir: &Path, local_make: &str) -> Self {
        Self::new(local_make, vec![entry.to_string()], Some(root_dir.to_path_buf()))
    }

    fn remote(target: &BuildTarget, entry: &str) -> Self {
        Self::new(
            "ssh",
            vec![
                "-tt".to_string(),
                target.host().to_string(),
                format!("cd '{}' && {} {}", target.remote_path(), target.make_cmd(), entry),
            ],
            None,
        )
    }
}

pub struct JobResult {
    log_path: PathBuf,
    status: i32,
}

impl JobResult {
    pub fn new(log_path: PathBuf, status: i32) -> Self {
        Self { log_path, status }
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn status(&self) -> i32 {
        self.status
    }

    pub fn is_success(&self) -> bool {
        self.status == 0
    }
}

struct PtySession<'a> {
    command: &'a BuildCommand,
}

impl<'a> PtySession<'a> {
    fn new(command: &'a BuildCommand) -> Result<Self, String> {
        Ok(Self { command })
    }

    fn run(&self, log_path: &Path) -> Result<i32, String> {
        self.pty_system()
            .openpty(self.size())
            .map_err(|err| format!("buildfarm: failed to open PTY: {err}"))
            .and_then(|pty| self.spawn(pty, log_path))
    }

    fn pty_system(&self) -> Box<dyn PtySystem + Send> {
        native_pty_system()
    }

    fn size(&self) -> PtySize {
        PtySize {
            rows: 40,
            cols: 160,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn spawn(&self, pty: portable_pty::PtyPair, log_path: &Path) -> Result<i32, String> {
        pty.slave
            .spawn_command(self.command_builder())
            .map_err(|err| format!("buildfarm: failed to spawn PTY child: {err}"))
            .and_then(|mut child| {
                drop(pty.slave);
                pty.master
                    .try_clone_reader()
                    .map_err(|err| format!("buildfarm: failed to clone PTY reader: {err}"))
                    .and_then(|reader| {
                        Self::capture(reader, log_path)
                            .and_then(|capture| child.wait().map_err(|err| format!("buildfarm: failed to wait for PTY child: {err}")).map(|status| (capture, status)))
                    })
                    .and_then(|(capture, status)| capture.join().map(|_| status))
            })
            .map(|status| i32::try_from(status.exit_code()).unwrap_or(1))
    }

    fn command_builder(&self) -> CommandBuilder {
        self.command
            .cwd()
            .map(|cwd| {
                let mut builder = CommandBuilder::new(self.command.program());
                builder.args(self.command.args());
                builder.cwd(cwd);
                builder.env("TERM", "xterm-256color");
                builder
            })
            .unwrap_or_else(|| {
                let mut builder = CommandBuilder::new(self.command.program());
                builder.args(self.command.args());
                builder.env("TERM", "xterm-256color");
                builder
            })
    }

    fn capture(reader: Box<dyn Read + Send>, log_path: &Path) -> Result<LogCapture, String> {
        File::create(log_path)
            .map_err(|err| format!("buildfarm: failed to create log file: {err}"))
            .map(|file| LogCapture::new(reader, file))
    }
}

struct LogCapture {
    thread: thread::JoinHandle<Result<(), String>>,
}

impl LogCapture {
    fn new(mut reader: Box<dyn Read + Send>, mut file: File) -> Self {
        Self {
            thread: thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader
                        .read(&mut buffer)
                        .map_err(|err| format!("buildfarm: failed to read PTY stream: {err}"))?
                    {
                        0 => return Ok(()),
                        read_len => {
                            file.write_all(&buffer[..read_len])
                                .map_err(|err| format!("buildfarm: failed to write log file: {err}"))?;
                        }
                    }
                }
            }),
        }
    }

    fn join(self) -> Result<(), String> {
        self.thread
            .join()
            .map_err(|_| "buildfarm: PTY log capture thread panicked".to_string())?
    }
}
