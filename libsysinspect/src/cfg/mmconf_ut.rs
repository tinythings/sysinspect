use super::mmconf::{DEFAULT_CONSOLE_PORT, MasterConfig};
use std::{fs, time::{SystemTime, UNIX_EPOCH}};

fn write_master_cfg(contents: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "sysinspect-mmconf-ut-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
    ));
    fs::create_dir_all(&base).unwrap();
    let path = base.join("sysinspect.conf");
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn master_console_defaults_are_used_when_not_configured() {
    let cfg = MasterConfig::new(write_master_cfg("config:\n  master:\n    fileserver.models: []\n")).unwrap();

    assert_eq!(cfg.console_bind_addr(), format!("127.0.0.1:{DEFAULT_CONSOLE_PORT}"));
}

#[test]
fn master_console_config_overrides_defaults() {
    let cfg = MasterConfig::new(write_master_cfg(
        "config:\n  master:\n    fileserver.models: []\n    console.bind.ip: 127.0.0.1\n    console.bind.port: 5511\n",
    ))
    .unwrap();

    assert_eq!(cfg.console_bind_addr(), "127.0.0.1:5511");
}
