use super::conf::EventsConfig;
use serde_yaml::Value;
use std::fs;

fn events_config_with_modules_path(path: &str) -> EventsConfig {
    let cfg: Value = serde_yaml::from_str(&format!("modules: {path}\nevents: {{}}\n"))
        .unwrap_or_else(|err| panic!("failed to build yaml config value: {err}"));

    EventsConfig::new(&cfg).unwrap_or_else(|err| panic!("failed to build events config: {err}"))
}

#[test]
fn events_config_does_not_resolve_python_files_as_modules() {
    let tmp = tempfile::Builder::new().prefix("sysinspect-events-conf-ut-").tempdir().unwrap_or_else(|err| panic!("failed to create tempdir: {err}"));

    fs::write(tmp.path().join("legacy.py"), "# legacy native python module\n")
        .unwrap_or_else(|err| panic!("failed to write legacy python file: {err}"));

    let cfg = events_config_with_modules_path(tmp.path().to_string_lossy().as_ref());

    let err = cfg.get_module("legacy").unwrap_err();
    assert!(err.to_string().contains("Missing module"));
}

#[test]
fn events_config_resolves_plain_module_paths_without_extension_fallback() {
    let tmp = tempfile::Builder::new().prefix("sysinspect-events-conf-ut-").tempdir().unwrap_or_else(|err| panic!("failed to create tempdir: {err}"));

    fs::create_dir_all(tmp.path().join("runtime")).unwrap_or_else(|err| panic!("failed to create runtime module dir: {err}"));

    let cfg = events_config_with_modules_path(tmp.path().to_string_lossy().as_ref());

    assert_eq!(cfg.get_module("runtime").unwrap_or_else(|err| panic!("expected module path: {err}")), tmp.path().join("runtime"));
}
