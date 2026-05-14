#[cfg(test)]
mod tests {
    use crate::oskernel::{Config, ManagerDef, current_os, parse_list_output, parse_operation, resolve_template, telemetry_base};
    use libmodcore::runtime;

    fn make_request(options: &[&str]) -> runtime::ModRequest {
        serde_json::from_value(serde_json::json!({ "options": options, "arguments": {} })).unwrap()
    }

    #[test]
    fn current_os_matches_target() {
        let os = current_os();
        assert!(!os.is_empty());
        if cfg!(target_os = "linux") {
            assert_eq!(os, "linux");
        }
    }

    #[test]
    fn parse_operation_returns_keys() {
        let mut resp = runtime::new_call_response();
        assert_eq!(parse_operation(&make_request(&["status"]), &mut resp), Some("status"));
        assert_eq!(parse_operation(&make_request(&["info"]), &mut resp), Some("info"));
        assert_eq!(parse_operation(&make_request(&["list"]), &mut resp), Some("list"));
        assert_eq!(parse_operation(&make_request(&["load"]), &mut resp), Some("load"));
        assert_eq!(parse_operation(&make_request(&["unload"]), &mut resp), Some("unload"));
    }

    #[test]
    fn parse_operation_none_is_error() {
        let rt = make_request(&[]);
        let mut resp = runtime::new_call_response();
        assert_eq!(parse_operation(&rt, &mut resp), None);
        assert_eq!(serde_json::to_value(&resp).unwrap()["retcode"], 1);
    }

    #[test]
    fn builtin_yaml_parses_all_oses() {
        let cfg = Config::from_merged(None).unwrap();
        assert!(cfg.managers.contains_key("freebsd-kld"));
        assert!(cfg.managers.contains_key("netbsd-mod"));
        assert!(cfg.managers.contains_key("openbsd-builtin"));
        assert!(cfg.managers.contains_key("linux-modprobe"));
        assert!(cfg.managers.contains_key("macos-kext"));
        assert!(cfg.managers.contains_key("solaris-mod"));
        assert!(cfg.managers.contains_key("android-mod"));
    }

    #[test]
    fn user_yaml_overrides_and_extends() {
        let user = "
managers:
  linux-modprobe:
    os: linux
    detect: fake-detect
    description: overridden
    load: fake-load {name}
    unload: fake-unload {name}
    status: fake-status {name}
    info: fake-info {name}
    list_modules: fake-list
  custom-kmod:
    os: linux
    detect: custom-detect
    description: custom
    load: custom-load {name}
    unload: custom-unload {name}
    status: custom-status {name}
    info: custom-info {name}
    list_modules: custom-list
";
        let cfg = Config::from_merged(Some(user)).unwrap();
        let systemd = cfg.managers.get("linux-modprobe").unwrap();
        assert_eq!(systemd.detect, "fake-detect");
        assert_eq!(systemd.load, "fake-load {name}");
        assert!(cfg.managers.contains_key("custom-kmod"));
    }

    fn test_manager() -> ManagerDef {
        ManagerDef {
            os: "linux".into(),
            detect: "true".into(),
            description: None,
            load: "load {name}".into(),
            unload: "unload {name}".into(),
            status: "status {name}".into(),
            info: "info {name}".into(),
            list_modules: "list".into(),
        }
    }

    #[test]
    fn resolve_template_load() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "load"), "load {name}");
    }

    #[test]
    fn resolve_template_info() {
        let mgr = test_manager();
        assert_eq!(resolve_template(&mgr, "info"), "info {name}");
    }

    #[test]
    fn telemetry_base_fields() {
        let data = telemetry_base("zfs", "freebsd-kld");
        assert_eq!(data.get("name").unwrap(), "zfs");
        assert_eq!(data.get("manager").unwrap(), "freebsd-kld");
    }

    #[test]
    fn parse_kldstat_list_output() {
        let out = " 1    1 0xffffffff80000000 12345    pf.ko\n 2    2 0xffffffff81000000 67890    zfs.ko\n";
        let mods = parse_list_output("freebsd-kld", out);
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0]["name"], "pf.ko");
        assert_eq!(mods[1]["name"], "zfs.ko");
        assert_eq!(mods[0]["loaded"], true);
    }

    #[test]
    fn parse_lsmod_list_output() {
        let out = "Module                  Size  Used by\nzfs                  3670016  0\npf                    123456  1\n";
        let mods = parse_list_output("linux-modprobe", out);
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0]["name"], "zfs");
        assert_eq!(mods[1]["name"], "pf");
        assert_eq!(mods[0]["size_bytes"], 3670016);
    }
}
