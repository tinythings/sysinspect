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
fn master_api_tls_relative_paths_are_resolved_under_root() {
    let cfg = MasterConfig::new(write_master_cfg(
        "config:\n  master:\n    fileserver.models: []\n    api.tls.enabled: true\n    api.tls.cert-file: etc/web/api.crt\n    api.tls.key-file: etc/web/api.key\n    api.tls.ca-file: trust/ca.pem\n    api.tls.allow-insecure: true\n",
    ))
    .unwrap();

    assert!(cfg.api_tls_enabled());
    assert_eq!(cfg.api_tls_cert_file().unwrap(), cfg.root_dir().join("etc/web/api.crt"));
    assert_eq!(cfg.api_tls_key_file().unwrap(), cfg.root_dir().join("etc/web/api.key"));
    assert_eq!(cfg.api_tls_ca_file().unwrap(), cfg.root_dir().join("trust/ca.pem"));
    assert!(cfg.api_tls_allow_insecure());
}

#[test]
fn master_api_tls_absolute_paths_stay_absolute() {
    let cfg = MasterConfig::new(write_master_cfg(
        "config:\n  master:\n    fileserver.models: []\n    api.tls.cert-file: /srv/tls/api.crt\n    api.tls.key-file: /srv/tls/api.key\n    api.tls.ca-file: /srv/tls/ca.pem\n",
    ))
    .unwrap();

    assert_eq!(cfg.api_tls_cert_file().unwrap(), std::path::PathBuf::from("/srv/tls/api.crt"));
    assert_eq!(cfg.api_tls_key_file().unwrap(), std::path::PathBuf::from("/srv/tls/api.key"));
    assert_eq!(cfg.api_tls_ca_file().unwrap(), std::path::PathBuf::from("/srv/tls/ca.pem"));
    assert!(!cfg.api_tls_enabled());
    assert!(!cfg.api_tls_allow_insecure());
}

#[test]
fn master_api_doc_defaults_preserve_current_behavior() {
    let cfg = MasterConfig::new(write_master_cfg("config:\n  master:\n    fileserver.models: []\n")).unwrap();

    assert!(cfg.api_doc_enabled());
}

#[test]
fn master_api_doc_config_overrides_defaults() {
    assert!(!MasterConfig::new(write_master_cfg("config:\n  master:\n    fileserver.models: []\n    api.doc: false\n")).unwrap().api_doc_enabled());
}

#[test]
fn minion_transport_paths_are_under_managed_transport_root() {
    let mut cfg = MinionConfig::default();
    cfg.set_root_dir("/srv/sysinspect");

    assert_eq!(cfg.transport_root(), cfg.root_dir().join(CFG_TRANSPORT_ROOT));
    assert_eq!(cfg.transport_master_root(), cfg.transport_root().join(CFG_TRANSPORT_MASTER));
    assert_eq!(cfg.transport_state_file(), cfg.transport_master_root().join(CFG_TRANSPORT_STATE));
}
