use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    model::{BuildTarget, BuildfarmConfig, ResultMirrorPlan},
    runner::{BuildCommand, BuildJob, BuildPlan},
};

#[test]
fn local_job_writes_full_log_file_from_pty() {
    let log_path = TempDir::new("buildfarm-runner-ut")
        .path()
        .join("local.log");
    let job = BuildJob::new(
        BuildTarget::local(),
        BuildCommand::new(
            "sh",
            vec!["-lc".to_string(), "printf '\\033[1;31mRED\\033[0m\\n'".to_string()],
            None,
        ),
        log_path.clone(),
        PathBuf::from("/tmp/sysinspect"),
        ResultMirrorPlan::disabled(PathBuf::from("/tmp/buildfarm"), "dev"),
    );
    let result = job.run().expect("local PTY job should run");

    assert!(result.is_success());
    assert_eq!(result.status(), 0);
    assert_eq!(result.log_path(), Path::new(&log_path));
    assert!(job.target().is_local());
    assert!(fs::read_to_string(&log_path).expect("log file should exist").contains("\u{1b}[1;31mRED"));
}

#[test]
fn remote_job_uses_ssh_tty_and_remote_make_command() {
    let job = BuildJob::build(
        &BuildTarget::remote("FreeBSD", "amd64", "192.168.122.122:work/sysinspect-buildfarm"),
        "dev",
        Path::new("/tmp/sysinspect"),
        Path::new("/tmp/logs"),
        "make",
        &ResultMirrorPlan::disabled(PathBuf::from("/tmp/buildfarm"), "dev"),
    );
    let command = job.command().args();

    assert_eq!(command[0], "-tt");
    assert_eq!(command[1], "192.168.122.122");
    assert_eq!(command[2], "cd 'work/sysinspect-buildfarm' && gmake dev");
}

#[test]
fn build_plan_creates_one_job_per_target_with_stable_log_paths() {
    let temp = TempDir::new("buildfarm-plan-ut");
    let plan = BuildPlan::new(
        &BuildfarmConfig::parse("local\nFreeBSD amd64 192.168.122.122:work/sysinspect-buildfarm\n")
            .expect("config should parse"),
        "modules-dev",
        Path::new("/tmp/sysinspect"),
        temp.path(),
        "make",
        ResultMirrorPlan::disabled(PathBuf::from("/tmp/buildfarm"), "modules-dev"),
    );

    assert_eq!(plan.jobs().len(), 2);
    assert_eq!(plan.jobs()[0].log_path(), temp.path().join("local.log"));
    assert_eq!(
        plan.jobs()[1].log_path(),
        temp.path().join("192.168.122.122_work_sysinspect-buildfarm.log")
    );
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        Self {
            path: std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock should move forward")
                    .as_nanos()
            )),
        }
        .create()
    }

    fn create(self) -> Self {
        fs::create_dir_all(&self.path).expect("temp dir should be created");
        self
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
