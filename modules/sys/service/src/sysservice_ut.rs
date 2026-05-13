#[cfg(test)]
mod tests {
    use crate::sysservice::{Config, ManagerDef, current_os, parse_operation, parse_status_output, resolve_template, telemetry_base};
    use libmodcore::runtime;

    fn make_request(options: &[&str]) -> runtime::ModRequest {
        serde_json::from_value(serde_json::json!({
            "options": options,
            "arguments": {}
        }))
        .unwrap()
    }

    #[test]
    fn current_os_matches_compile_target() {
        let os = current_os();
        if cfg!(target_os = "linux") {
            assert_eq!(os, "linux");
        } else if cfg!(target_os = "freebsd") {
            assert_eq!(os, "freebsd");
        } else if cfg!(target_os = "openbsd") {
            assert_eq!(os, "openbsd");
        } else if cfg!(target_os = "netbsd") {
            assert_eq!(os, "netbsd");
        } else if cfg!(target_os = "macos") {
            assert_eq!(os, "macos");
        }
        assert!(!os.is_empty());
    }

    #[test]
    fn parse_operation_returns_correct_keys() {
        let mut resp = runtime::new_call_response();
        assert_eq!(parse_operation(&make_request(&["check"]), &mut resp), Some("check"));
        assert_eq!(parse_operation(&make_request(&["status"]), &mut resp), Some("status"));
        assert_eq!(parse_operation(&make_request(&["start"]), &mut resp), Some("start"));
        assert_eq!(parse_operation(&make_request(&["stop"]), &mut resp), Some("stop"));
        assert_eq!(parse_operation(&make_request(&["restart"]), &mut resp), Some("restart"));
        assert_eq!(parse_operation(&make_request(&["reload"]), &mut resp), Some("reload"));
        assert_eq!(parse_operation(&make_request(&["enable"]), &mut resp), Some("enable"));
        assert_eq!(parse_operation(&make_request(&["disable"]), &mut resp), Some("disable"));
    }

    #[test]
    fn parse_operation_none_is_error() {
        let rt = make_request(&[]);
        let mut resp = runtime::new_call_response();
        assert_eq!(parse_operation(&rt, &mut resp), None);
        assert_eq!(serde_json::to_value(&resp).unwrap()["retcode"], 1);
    }

    #[test]
    fn builtin_yaml_parses() {
        let cfg = Config::from_merged(None).unwrap();
        assert!(cfg.managers.contains_key("freebsd-rcd"));
        assert!(cfg.managers.contains_key("openbsd-rcctl"));
        assert!(cfg.managers.contains_key("linux-systemd"));
        assert!(cfg.managers.contains_key("linux-sysv"));
        assert!(cfg.managers.contains_key("android-prop"));
    }

    #[test]
    fn user_yaml_overrides_builtin() {
        let user = "
managers:
  linux-systemd:
    os: linux
    detect: my-fake-detect
    start: fake-start {name}
    stop: fake-stop {name}
    restart: fake-restart {name}
    status: fake-status {name}
  custom-mgr:
    os: linux
    detect: custom-detect
    start: custom-start {name}
    stop: custom-stop {name}
    restart: custom-restart {name}
    status: custom-status {name}
";
        let cfg = Config::from_merged(Some(user)).unwrap();
        let systemd = cfg.managers.get("linux-systemd").unwrap();
        assert_eq!(systemd.detect, "my-fake-detect");
        assert_eq!(systemd.start, "fake-start {name}");
        let custom = cfg.managers.get("custom-mgr").unwrap();
        assert_eq!(custom.detect, "custom-detect");
    }

    #[test]
    fn invalid_user_yaml_is_error() {
        assert!(Config::from_merged(Some(": bad yaml: :")).is_err());
    }

    fn test_manager() -> ManagerDef {
        ManagerDef {
            os: "linux".into(),
            detect: "true".into(),
            description: None,
            start: "start {name}".into(),
            stop: "stop {name}".into(),
            restart: "restart {name}".into(),
            reload: Some("reload {name}".into()),
            status: "status {name}".into(),
            enable: Some("enable {name}".into()),
            disable: None,
        }
    }

    #[test]
    fn resolve_template_start() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "start"), Some("start {name}"));
    }

    #[test]
    fn resolve_template_status_is_check_too() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "status"), Some("status {name}"));
        assert_eq!(resolve_template(&mgr, "check"), Some("status {name}"));
    }

    #[test]
    fn resolve_template_missing_optional() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "disable"), None);
    }

    #[test]
    fn resolve_template_unknown() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "nonsense"), None);
    }

    #[test]
    fn telemetry_base_has_required_fields() {
        let data = telemetry_base("sshd", "linux-systemd");
        assert_eq!(data.get("name").unwrap(), "sshd");
        assert_eq!(data.get("manager").unwrap(), "linux-systemd");
    }

    #[test]
    fn parse_status_for_systemd() {
        assert!(parse_status_output("linux-systemd", ""));
        assert!(parse_status_output("linux-systemd", "inactive"));
    }

    #[test]
    fn parse_status_for_rcd() {
        assert!(parse_status_output("freebsd-rcd", "sshd is running as pid 1234."));
        assert!(!parse_status_output("freebsd-rcd", "sshd is not running."));
    }
}
