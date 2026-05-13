#[cfg(test)]
mod tests {
    use crate::sysservice::{
        Config, ManagerDef, current_os, parse_info_output, parse_operation, parse_status_output, resolve_template, telemetry_base,
    };
    use libmodcore::runtime;

    fn make_request(options: &[&str]) -> runtime::ModRequest {
        serde_json::from_value(serde_json::json!({ "options": options, "arguments": {} })).unwrap()
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
        assert_eq!(parse_operation(&make_request(&["info"]), &mut resp), Some("info"));
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
        assert!(cfg.managers.contains_key("solaris-smf"));
    }

    #[test]
    fn user_yaml_overrides_builtin() {
        let user = "\nmanagers:\n  linux-systemd:\n    os: linux\n    detect: my-fake-detect\n    start: fake-start {name}\n    stop: fake-stop {name}\n    restart: fake-restart {name}\n    status: fake-status {name}\n  custom-mgr:\n    os: linux\n    detect: custom-detect\n    start: custom-start {name}\n    stop: custom-stop {name}\n    restart: custom-restart {name}\n    status: custom-status {name}\n";
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
            info: None,
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
    fn resolve_template_info_falls_back_to_status() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "info"), Some("status {name}"));
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

    #[test]
    fn parse_systemd_show_extracts_fields() {
        let mut data = telemetry_base("test", "linux-systemd");
        parse_info_output(
            "linux-systemd",
            "LoadState=loaded\nActiveState=active\nSubState=running\nFragmentPath=/lib/systemd/system/test.service\nDescription=Test Service\n",
            &mut data,
        );
        assert_eq!(data.get("load_state").unwrap(), "loaded");
        assert_eq!(data.get("active_state").unwrap(), "active");
        assert_eq!(data.get("sub_state").unwrap(), "running");
        assert_eq!(data.get("unit_path").unwrap(), "/lib/systemd/system/test.service");
        assert_eq!(data.get("description").unwrap(), "Test Service");
    }

    #[test]
    fn parse_smf_extracts_fields() {
        let mut data = telemetry_base("test", "solaris-smf");
        parse_info_output("solaris-smf", "state      online\nenabled    true\n", &mut data);
        assert_eq!(data.get("smf_state").unwrap(), "online");
        assert_eq!(data.get("enabled").unwrap(), "true");
    }
}
