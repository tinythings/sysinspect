use super::{running_minion_targets, stop_targets};
use libsysinspect::cfg::mmconf::MinionConfig;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn scratch_pidfile() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sysminion-start-ut-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.join("sysminion.pid")
}

#[test]
fn stop_targets_merge_pidfile_and_sniffed_without_self() {
    let pidfile = scratch_pidfile();
    fs::write(&pidfile, "42\n").unwrap();
    let mut cfg = MinionConfig::default();
    cfg.set_pid_path(pidfile.to_str().unwrap());

    assert_eq!(stop_targets(&cfg, &[42, 43, 77], 77), vec![42, 43]);

    let _ = fs::remove_file(pidfile);
}

#[test]
fn running_targets_merge_pidfile_and_process_scan_without_self() {
    let pidfile = scratch_pidfile();
    fs::write(&pidfile, "42\n").unwrap();
    let mut cfg = MinionConfig::default();
    cfg.set_pid_path(pidfile.to_str().unwrap());

    assert_eq!(running_minion_targets(&cfg, &[42, 42, 77, 91], 77), vec![42, 91]);

    let _ = fs::remove_file(pidfile);
}
