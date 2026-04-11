use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use crate::model::{BuildTarget, BuildfarmConfig, ResultMirrorPlan};
use portable_pty::{CommandBuilder, PtySize, PtySystem, native_pty_system};

pub struct BuildPlan {
    jobs: Vec<BuildJob>,
}

impl BuildPlan {
    pub fn new(config: &BuildfarmConfig, entry: &str, root_dir: &Path, log_root: &Path, local_make: &str, mirror_plan: ResultMirrorPlan) -> Self {
        Self { jobs: config.targets().iter().map(|target| BuildJob::build(target, entry, root_dir, log_root, local_make, &mirror_plan)).collect() }
    }

    pub fn jobs(&self) -> &[BuildJob] {
        &self.jobs
    }
}

pub struct BuildJob {
    target: BuildTarget,
    command: BuildCommand,
    log_path: PathBuf,
    root_dir: PathBuf,
    mirror_plan: ResultMirrorPlan,
}

impl Clone for BuildJob {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            command: self.command.clone(),
            log_path: self.log_path.clone(),
            root_dir: self.root_dir.clone(),
            mirror_plan: self.mirror_plan.clone(),
        }
    }
}

impl BuildJob {
    pub fn new(target: BuildTarget, command: BuildCommand, log_path: PathBuf, root_dir: PathBuf, mirror_plan: ResultMirrorPlan) -> Self {
        Self { target, command, log_path, root_dir, mirror_plan }
    }

    pub fn build(target: &BuildTarget, entry: &str, root_dir: &Path, log_root: &Path, local_make: &str, mirror_plan: &ResultMirrorPlan) -> Self {
        Self::new(
            target.clone(),
            BuildCommand::for_target(target, entry, root_dir, local_make),
            log_root.join(format!("{}.log", target.log_key())),
            root_dir.to_path_buf(),
            mirror_plan.clone(),
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
        self.reset_log_file()?;
        self.prepare().and_then(|_| {
            self.run_build().and_then(|status| {
                (status == 0)
                    .then_some(self.run_mirror().map(|_| JobResult::new(self.log_path.clone(), status)))
                    .unwrap_or_else(|| Ok(JobResult::new(self.log_path.clone(), status)))
            })
        })
    }

    fn reset_log_file(&self) -> Result<(), String> {
        File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.log_path)
            .map_err(|err| format!("buildfarm: failed to reset log file: {err}"))
            .map(|_| ())
    }

    pub(crate) fn prepare(&self) -> Result<(), String> {
        fs::create_dir_all(self.log_path.parent().unwrap_or_else(|| Path::new(".")))
            .map_err(|err| format!("buildfarm: failed to create log directory: {err}"))?;
        self.target.is_local().then_some(Ok(())).unwrap_or_else(|| RemoteSync::new(&self.root_dir, &self.target).run(&self.log_path))
    }

    pub(crate) fn run_build(&self) -> Result<i32, String> {
        self.command.run(&self.log_path)
    }

    pub(crate) fn run_mirror(&self) -> Result<(), String> {
        self.mirror_plan
            .is_enabled()
            .then_some(ResultMirror::new(&self.root_dir, &self.target, &self.mirror_plan).run(&self.log_path))
            .unwrap_or_else(|| Ok(()))
    }

    pub(crate) fn should_mirror_results(&self) -> bool {
        self.mirror_plan.is_enabled()
    }
}

pub struct BuildCommand {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
}

impl Clone for BuildCommand {
    fn clone(&self) -> Self {
        Self { program: self.program.clone(), args: self.args.clone(), cwd: self.cwd.clone() }
    }
}

impl BuildCommand {
    pub fn new(program: &str, args: Vec<String>, cwd: Option<PathBuf>) -> Self {
        Self { program: program.to_string(), args, cwd }
    }

    pub fn for_target(target: &BuildTarget, entry: &str, root_dir: &Path, local_make: &str) -> Self {
        target.is_local().then_some(Self::local(entry, root_dir, local_make)).unwrap_or_else(|| Self::remote(target, entry, root_dir))
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
        Self::new(
            "sh",
            vec!["-lc".to_string(), format!("BUILDFARM_CONFIG= BUILDFARM_LOCAL_MAKE= {} {}", local_make, entry)],
            Some(root_dir.to_path_buf()),
        )
    }

    fn remote(target: &BuildTarget, entry: &str, root_dir: &Path) -> Self {
        let _ = root_dir;
        Self::new(
            "ssh",
            vec!["-tt".to_string(), target.host().to_string(), format!("cd '{}' && {} {}", target.remote_path(), target.make_cmd(), entry)],
            None,
        )
    }
}

struct RemoteSync<'a> {
    root_dir: &'a Path,
    target: &'a BuildTarget,
}

impl<'a> RemoteSync<'a> {
    fn new(root_dir: &'a Path, target: &'a BuildTarget) -> Self {
        Self { root_dir, target }
    }

    fn run(&self, log_path: &Path) -> Result<(), String> {
        self.ensure_remote_dir(log_path)?;
        self.rsync_tree(log_path)
    }

    fn ensure_remote_dir(&self, log_path: &Path) -> Result<(), String> {
        self.command("ssh", self.ensure_remote_dir_args()).status(log_path).and_then(Self::ensure_success)
    }

    fn rsync_tree(&self, log_path: &Path) -> Result<(), String> {
        self.command("rsync", self.rsync_args()).status(log_path).and_then(Self::ensure_success)
    }

    fn ensure_remote_dir_args(&self) -> Vec<String> {
        vec![self.target.host().to_string(), format!("mkdir -p '{}'", self.target.remote_path())]
    }

    fn rsync_args(&self) -> Vec<String> {
        vec![
            "-az".to_string(),
            "--exclude".to_string(),
            ".git".to_string(),
            "--exclude".to_string(),
            ".github".to_string(),
            "--exclude".to_string(),
            ".vscode".to_string(),
            "--exclude".to_string(),
            ".idea".to_string(),
            "--exclude".to_string(),
            ".buildfarm".to_string(),
            "--exclude".to_string(),
            "target".to_string(),
            "--exclude".to_string(),
            "build/stage".to_string(),
            "--exclude".to_string(),
            "build/modules-dist".to_string(),
            format!("{}/", self.root_dir.display()),
            format!("{}:{}/", self.target.host(), self.target.remote_path()),
        ]
    }

    fn command(&self, program: &str, args: Vec<String>) -> LoggedCommand {
        LoggedCommand::new(program, args)
    }

    fn ensure_success(status: i32) -> Result<(), String> {
        (status == 0).then_some(()).ok_or_else(|| "buildfarm: remote sync failed".to_string())
    }
}

struct ResultMirror<'a> {
    root_dir: &'a Path,
    target: &'a BuildTarget,
    mirror_plan: &'a ResultMirrorPlan,
}

impl<'a> ResultMirror<'a> {
    fn new(root_dir: &'a Path, target: &'a BuildTarget, mirror_plan: &'a ResultMirrorPlan) -> Self {
        Self { root_dir, target, mirror_plan }
    }

    fn run(&self, log_path: &Path) -> Result<(), String> {
        let mirror_target = self.resolve_target()?;

        self.layout_roots()
            .is_empty()
            .then_some(Ok(()))
            .unwrap_or_else(|| {
                self.log_mirror_header(&mirror_target, log_path).and_then(|_| {
                    self.layout_roots().iter().enumerate().try_for_each(|(index, root)| {
                        self.ensure_target_subdir(&mirror_target, root)
                            .and_then(|_| self.log_mirror_root(&mirror_target, index + 1, root, log_path))
                            .and_then(|_| self.mirror_root(&mirror_target, root, log_path))
                    })
                })
            })
    }

    fn resolve_target(&self) -> Result<MirrorTarget, String> {
        self.resolve_os_label()
            .map(|label| MirrorTarget::new(label, self.mirror_plan.root()))
    }

    fn resolve_os_label(&self) -> Result<String, String> {
        self.is_linux_target()
            .then_some(self.resolve_linux_label())
            .unwrap_or_else(|| self.resolve_bsd_label())
    }

    fn resolve_linux_label(&self) -> Result<String, String> {
        self.kernel_major()
            .and_then(|kernel| self.libc_label().map(|libc| format!("Linux {kernel} {libc}")))
    }

    fn resolve_bsd_label(&self) -> Result<String, String> {
        self.release_major_minor()
            .map(|release| format!("{} {release}", self.target.os()))
    }

    fn kernel_major(&self) -> Result<String, String> {
        self.capture("uname", &["-r"]).map(|release| {
            release
                .split('.')
                .next()
                .unwrap_or(release.as_str())
                .to_string()
        })
    }

    fn release_major_minor(&self) -> Result<String, String> {
        self.capture("uname", &["-r"]).map(|release| {
            release
                .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
                .find(|part| !part.is_empty())
                .map(Self::take_major_minor)
                .unwrap_or(release)
        })
    }

    fn libc_label(&self) -> Result<String, String> {
        self.glibc_label()
            .or_else(|_| self.musl_label())
    }

    fn glibc_label(&self) -> Result<String, String> {
        self.capture("getconf", &["GNU_LIBC_VERSION"]).and_then(|value| {
            value
                .strip_prefix("glibc ")
                .map(|version| format!("glibc {version}"))
                .ok_or_else(|| "buildfarm: could not parse glibc version".to_string())
        })
    }

    fn musl_label(&self) -> Result<String, String> {
        self.capture("ldd", &["--version"]).and_then(|value| {
            Self::first_version(&value)
                .map(|version| format!("musl {version}"))
                .ok_or_else(|| "buildfarm: could not parse musl version".to_string())
        })
    }

    fn capture(&self, program: &str, args: &[&str]) -> Result<String, String> {
        self.target
            .is_local()
            .then_some(Self::capture_local(program, args))
            .unwrap_or_else(|| Self::capture_remote(self.target.host(), program, args))
    }

    fn capture_local(program: &str, args: &[&str]) -> Result<String, String> {
        Command::new(program)
            .args(args)
            .output()
            .map_err(|err| format!("buildfarm: failed to run '{program}': {err}"))
            .and_then(|output| Self::capture_output(program, output))
    }

    fn capture_remote(host: &str, program: &str, args: &[&str]) -> Result<String, String> {
        Command::new("ssh")
            .arg(host)
            .arg(program)
            .args(args)
            .output()
            .map_err(|err| format!("buildfarm: failed to query {host}: {err}"))
            .and_then(|output| Self::capture_output(program, output))
    }

    fn capture_output(program: &str, output: std::process::Output) -> Result<String, String> {
        output
            .status
            .success()
            .then_some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                format!("buildfarm: failed to query '{program}': {stderr}")
            })
    }

    fn first_version(text: &str) -> Option<String> {
        text.split_whitespace()
            .find(|part| {
                part.chars().any(|ch| ch.is_ascii_digit())
                    && part.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
            })
            .map(|part| part.to_string())
    }

    fn take_major_minor(release: &str) -> String {
        release
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn is_linux_target(&self) -> bool {
        matches!(self.target.os(), "GNU/Linux" | "Linux")
            || self.target.os().starts_with("GNU/Linux")
            || self.target.os().starts_with("Linux")
    }

    fn log_mirror_header(&self, mirror_target: &MirrorTarget, log_path: &Path) -> Result<(), String> {
        LogAppendFile::open(log_path).and_then(|mut log_file| {
            log_file.write_text(&format!(
                "\n\nMirroring {} artifacts:\n",
                Self::blue_bold(mirror_target.label())
            ))
        })
    }

    fn ensure_target_subdir(&self, mirror_target: &MirrorTarget, root: &Path) -> Result<(), String> {
        fs::create_dir_all(mirror_target.root().join(root)).map_err(|err| format!("buildfarm: failed to create mirror directory: {err}"))
    }

    fn mirror_root(&self, mirror_target: &MirrorTarget, root: &Path, log_path: &Path) -> Result<(), String> {
        self.target.is_local().then_some(self.mirror_local_root(mirror_target, root, log_path)).unwrap_or_else(|| self.mirror_remote_root(mirror_target, root, log_path))
    }

    fn mirror_local_root(&self, mirror_target: &MirrorTarget, root: &Path, log_path: &Path) -> Result<(), String> {
        self.command("rsync", self.local_rsync_args(mirror_target, root)).status(log_path).and_then(Self::ensure_success)
    }

    fn mirror_remote_root(&self, mirror_target: &MirrorTarget, root: &Path, log_path: &Path) -> Result<(), String> {
        self.command("rsync", self.remote_rsync_args(mirror_target, root)).status(log_path).and_then(Self::ensure_success)
    }

    fn log_mirror_root(&self, mirror_target: &MirrorTarget, index: usize, root: &Path, log_path: &Path) -> Result<(), String> {
        LogAppendFile::open(log_path).and_then(|mut log_file| {
            log_file.write_text(&format!(
                "    {index}. src: {}\n         dst: {}\n\n",
                Self::green(&self.source_label(root)),
                Self::green(&mirror_target.root().join(root).display().to_string())
            ))
        })
    }

    fn local_rsync_args(&self, mirror_target: &MirrorTarget, root: &Path) -> Vec<String> {
        vec![
            "-az".to_string(),
            "--delete".to_string(),
            format!("{}/", self.source_root(root).display()),
            format!("{}/", mirror_target.root().join(root).display()),
        ]
    }

    fn remote_rsync_args(&self, mirror_target: &MirrorTarget, root: &Path) -> Vec<String> {
        vec![
            "-az".to_string(),
            "--delete".to_string(),
            format!("{}:{}/", self.target.host(), self.remote_source_root(root)),
            format!("{}/", mirror_target.root().join(root).display()),
        ]
    }

    fn source_root(&self, root: &Path) -> PathBuf {
        self.root_dir.join(root)
    }

    fn remote_source_root(&self, root: &Path) -> String {
        format!("{}/{}", self.target.remote_path(), root.display())
    }

    fn source_label(&self, root: &Path) -> String {
        self.target
            .is_local()
            .then_some(self.source_root(root).display().to_string())
            .unwrap_or_else(|| format!("{}:{}", self.target.host(), self.remote_source_root(root)))
    }

    fn layout_roots(&self) -> &[PathBuf] {
        self.mirror_plan.layout().roots()
    }

    fn blue_bold(text: &str) -> String {
        format!("\u{1b}[1;94m{text}\u{1b}[0m")
    }

    fn green(text: &str) -> String {
        format!("\u{1b}[92m{text}\u{1b}[0m")
    }

    fn command(&self, program: &str, args: Vec<String>) -> LoggedCommand {
        LoggedCommand::new(program, args)
    }

    fn ensure_success(status: i32) -> Result<(), String> {
        (status == 0).then_some(()).ok_or_else(|| "buildfarm: result mirroring failed".to_string())
    }
}

struct MirrorTarget {
    label: String,
    root: PathBuf,
}

impl MirrorTarget {
    fn new(label: String, mirror_root: &Path) -> Self {
        Self {
            root: mirror_root.join(Self::dir_name(&label)),
            label,
        }
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn dir_name(label: &str) -> String {
        label
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '.' {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }
}

struct LoggedCommand {
    program: String,
    args: Vec<String>,
}

impl LoggedCommand {
    fn new(program: &str, args: Vec<String>) -> Self {
        Self { program: program.to_string(), args }
    }

    fn status(&self, log_path: &Path) -> Result<i32, String> {
        LogAppendFile::open(log_path).and_then(|log_file| {
            log_file
                .spawn(&self.program, &self.args)
                .and_then(|mut child| child.wait().map_err(|err| format!("buildfarm: failed to wait for command: {err}")))
                .map(|status| status.code().unwrap_or(1))
        })
    }
}

struct LogAppendFile {
    file: File,
}

impl LogAppendFile {
    fn open(log_path: &Path) -> Result<Self, String> {
        File::options()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|err| format!("buildfarm: failed to open log file for append: {err}"))
            .map(|file| Self { file })
    }

    fn spawn(&self, program: &str, args: &[String]) -> Result<std::process::Child, String> {
        let stdout = self.file.try_clone().map_err(|err| format!("buildfarm: failed to clone log file handle: {err}"))?;
        let stderr = self.file.try_clone().map_err(|err| format!("buildfarm: failed to clone log file handle: {err}"))?;

        Command::new(program)
            .args(args)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|err| format!("buildfarm: failed to spawn command '{program}': {err}"))
    }

    fn write_line(&mut self, line: &str) -> Result<(), String> {
        self.write_text(&format!("{line}\n"))
    }

    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.file
            .write_all(text.as_bytes())
            .map_err(|err| format!("buildfarm: failed to append log file: {err}"))
            .and_then(|_| self.file.flush().map_err(|err| format!("buildfarm: failed to flush log file: {err}")))
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
        PtySize { rows: 40, cols: 160, pixel_width: 0, pixel_height: 0 }
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
                        Self::capture(reader, log_path).and_then(|capture| {
                            child.wait().map_err(|err| format!("buildfarm: failed to wait for PTY child: {err}")).map(|status| (capture, status))
                        })
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
        File::create(log_path).map_err(|err| format!("buildfarm: failed to create log file: {err}")).map(|file| LogCapture::new(reader, file))
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
                    match reader.read(&mut buffer).map_err(|err| format!("buildfarm: failed to read PTY stream: {err}"))? {
                        0 => return Ok(()),
                        read_len => {
                            file.write_all(&buffer[..read_len]).map_err(|err| format!("buildfarm: failed to write log file: {err}"))?;
                            file.flush().map_err(|err| format!("buildfarm: failed to flush log file: {err}"))?;
                        }
                    }
                }
            }),
        }
    }

    fn join(self) -> Result<(), String> {
        self.thread.join().map_err(|_| "buildfarm: PTY log capture thread panicked".to_string())?
    }
}
