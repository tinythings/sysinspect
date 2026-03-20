use super::mmconf::{
    CFG_TRANSPORT_MASTER, CFG_TRANSPORT_MINIONS, CFG_TRANSPORT_ROOT, CFG_TRANSPORT_STATE, DEFAULT_CONSOLE_PORT, MasterConfig, MinionConfig,
};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

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

    assert_eq!(cfg.console_listen_addr(), format!("127.0.0.1:{DEFAULT_CONSOLE_PORT}"));
    assert_eq!(cfg.console_connect_addr(), format!("127.0.0.1:{DEFAULT_CONSOLE_PORT}"));
}

#[test]
fn master_console_config_overrides_defaults() {
    let cfg = MasterConfig::new(write_master_cfg(
        "config:\n  master:\n    fileserver.models: []\n    console.bind.ip: 127.0.0.1\n    console.bind.port: 5511\n",
    ))
    .unwrap();

    assert_eq!(cfg.console_listen_addr(), "127.0.0.1:5511");
    assert_eq!(cfg.console_connect_addr(), "127.0.0.1:5511");
}

#[test]
fn master_console_connect_addr_rewrites_wildcard_bind_to_loopback() {
    let cfg = MasterConfig::new(write_master_cfg(
        "config:\n  master:\n    fileserver.models: []\n    console.bind.ip: 0.0.0.0\n    console.bind.port: 5511\n",
    ))
    .unwrap();

    assert_eq!(cfg.console_listen_addr(), "0.0.0.0:5511");
    assert_eq!(cfg.console_connect_addr(), "127.0.0.1:5511");
}

#[test]
fn master_transport_paths_are_under_managed_transport_root() {
    let cfg = MasterConfig::new(write_master_cfg("config:\n  master:\n    fileserver.models: []\n")).unwrap();

    assert_eq!(cfg.transport_root(), cfg.root_dir().join(CFG_TRANSPORT_ROOT));
    assert_eq!(cfg.transport_minions_root(), cfg.transport_root().join(CFG_TRANSPORT_MINIONS));
}

#[test]
fn minion_transport_paths_are_under_managed_transport_root() {
    let mut cfg = MinionConfig::default();
    cfg.set_root_dir("/srv/sysinspect");

    assert_eq!(cfg.transport_root(), cfg.root_dir().join(CFG_TRANSPORT_ROOT));
    assert_eq!(cfg.transport_master_root(), cfg.transport_root().join(CFG_TRANSPORT_MASTER));
    assert_eq!(cfg.transport_state_file(), cfg.transport_master_root().join(CFG_TRANSPORT_STATE));
}
