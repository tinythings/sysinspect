use libsysinspect::cfg::mmconf::{MasterConfig, MinionConfig};
use std::fs;
use tempfile::Builder;

fn write_config(dir: &std::path::Path, contents: &str) {
    fs::write(dir.join("sysinspect.conf"), contents).unwrap();
}

fn write_dropin(dir: &std::path::Path, name: &str, contents: &str) {
    let dd = dir.join("sysinspect.d");
    fs::create_dir_all(&dd).unwrap();
    fs::write(dd.join(name), contents).unwrap();
}

#[test]
fn no_dropins_dir_parses_cleanly() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    fileserver.models: [foo]\n    bind.ip: 0.0.0.0\n    bind.port: 4200\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.fileserver_models().as_slice(), &["foo"]);
    assert_eq!(cfg.bind_addr(), "0.0.0.0:4200");
}

#[test]
fn scalar_override_replaces_value() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    bind.ip: 0.0.0.0\n    bind.port: 4200\n    fileserver.models: []\n");
    write_dropin(tmp.path(), "override.yml", "config:\n  master:\n    bind.port: 9999\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.bind_addr(), "0.0.0.0:9999");
}

#[test]
fn nested_map_merge_adds_key() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(
        tmp.path(),
        "config:\n  master:\n    fileserver.models: []\n    telemetry.location: /default/path\n    telemetry.socket: /default/sock\n",
    );
    write_dropin(tmp.path(), "override.yml", "config:\n  master:\n    telemetry.location: /overridden/path\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.telemetry_location(), std::path::PathBuf::from("/overridden/path"));
    assert_eq!(cfg.telemetry_socket(), std::path::PathBuf::from("/default/sock"));
}

#[test]
fn sequence_append_extends_list() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    fileserver.models: [a]\n");
    write_dropin(tmp.path(), "01-models.yml", "config:\n  master:\n    fileserver.models: [b]\n");
    write_dropin(tmp.path(), "02-models.yml", "config:\n  master:\n    fileserver.models: [c]\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.fileserver_models().as_slice(), &["a", "b", "c"]);
}

#[test]
fn ordering_later_file_wins_conflict() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    bind.ip: 0.0.0.0\n    bind.port: 4200\n    fileserver.models: []\n");
    write_dropin(tmp.path(), "01-a.yml", "config:\n  master:\n    bind.port: 1111\n");
    write_dropin(tmp.path(), "02-b.yml", "config:\n  master:\n    bind.port: 2222\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.bind_addr(), "0.0.0.0:2222");
}

#[test]
fn non_yaml_files_ignored() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    fileserver.models: [only-this]\n");
    let dd = tmp.path().join("sysinspect.d");
    fs::create_dir_all(&dd).unwrap();
    fs::write(dd.join("notes.txt"), "garbage").unwrap();
    fs::write(dd.join("data.json"), r#"{"x": 1}"#).unwrap();
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.fileserver_models().as_slice(), &["only-this"]);
}

#[test]
fn empty_dropins_dir_no_effect() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    fileserver.models: [x]\n");
    fs::create_dir_all(tmp.path().join("sysinspect.d")).unwrap();
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.fileserver_models().as_slice(), &["x"]);
}

#[test]
fn new_key_added_by_dropin() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  master:\n    fileserver.models: []\n");
    write_dropin(tmp.path(), "extra.yml", "config:\n  master:\n    api.enabled: true\n    api.bind.port: 8080\n");
    let cfg = MasterConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert!(cfg.api_enabled());
    assert_eq!(cfg.api_bind_port(), 8080);
}

#[test]
fn minion_config_dropins_work() {
    let tmp = Builder::new().prefix("dropins-int-").tempdir().unwrap();
    write_config(tmp.path(), "config:\n  minion:\n    master.ip: 10.0.0.1\n    master.port: 4200\n    performance: embedded\n");
    write_dropin(tmp.path(), "override.yml", "config:\n  minion:\n    master.port: 9999\n");
    let cfg = MinionConfig::new(tmp.path().join("sysinspect.conf")).unwrap();
    assert_eq!(cfg.master(), "10.0.0.1:9999");
}
