#[cfg(test)]
mod tests {
    use crate::syspkg;
    use libmodcore::runtime;

    fn make_request(options: &[&str]) -> libmodcore::runtime::ModRequest {
        serde_json::from_value(serde_json::json!({
            "options": options,
            "arguments": {}
        }))
        .unwrap()
    }

    #[test]
    fn get_operation_check() {
        let rt = make_request(&["check"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "check");
    }

    #[test]
    fn get_operation_install() {
        let rt = make_request(&["install"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "install");
    }

    #[test]
    fn get_operation_remove() {
        let rt = make_request(&["remove"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "remove");
    }

    #[test]
    fn get_operation_update() {
        let rt = make_request(&["update"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "update");
    }

    #[test]
    fn get_operation_upgrade() {
        let rt = make_request(&["upgrade"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "upgrade");
    }

    #[test]
    fn get_operation_search() {
        let rt = make_request(&["search"]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "search");
    }

    #[test]
    fn get_operation_none_is_error() {
        let rt = make_request(&[]);
        let mut resp = runtime::new_call_response();
        assert_eq!(syspkg::get_operation(&rt, &mut resp), "");
        assert_eq!(serde_json::to_value(&resp).unwrap()["retcode"], 1);
    }

    // ---------------------------------------------------------------
    // OS-specific command generation tests
    // ---------------------------------------------------------------

    #[cfg(target_os = "freebsd")]
    mod freebsd {
        use crate::syspkg::get_pkg_command;

        #[test]
        fn install_command() {
            let (cmd, args) = get_pkg_command("install", "nginx").unwrap();
            assert_eq!(cmd, "pkg");
            assert_eq!(args, vec!["install", "-y", "nginx"]);
        }

        #[test]
        fn remove_command() {
            let (cmd, args) = get_pkg_command("remove", "nginx").unwrap();
            assert_eq!(cmd, "pkg");
            assert_eq!(args, vec!["delete", "-y", "nginx"]);
        }

        #[test]
        fn update_command() {
            let (cmd, args) = get_pkg_command("update", "").unwrap();
            assert_eq!(cmd, "pkg");
            assert_eq!(args, vec!["update"]);
        }

        #[test]
        fn upgrade_command() {
            let (cmd, args) = get_pkg_command("upgrade", "").unwrap();
            assert_eq!(cmd, "pkg");
            assert_eq!(args, vec!["upgrade", "-y"]);
        }

        #[test]
        fn search_command() {
            let (cmd, args) = get_pkg_command("search", "nginx").unwrap();
            assert_eq!(cmd, "pkg");
            assert_eq!(args, vec!["search", "nginx"]);
        }

        #[test]
        fn unknown_operation_is_error() {
            assert!(get_pkg_command("nuke", "foo").is_err());
        }
    }

    #[cfg(target_os = "openbsd")]
    mod openbsd {
        use crate::syspkg::get_pkg_command;

        #[test]
        fn install_command() {
            let (cmd, args) = get_pkg_command("install", "nginx").unwrap();
            assert_eq!(cmd, "pkg_add");
            assert_eq!(args, vec!["nginx"]);
        }

        #[test]
        fn remove_command() {
            let (cmd, args) = get_pkg_command("remove", "nginx").unwrap();
            assert_eq!(cmd, "pkg_delete");
            assert_eq!(args, vec!["nginx"]);
        }

        #[test]
        fn update_is_error() {
            assert!(get_pkg_command("update", "").is_err());
        }

        #[test]
        fn upgrade_command() {
            let (cmd, args) = get_pkg_command("upgrade", "nginx").unwrap();
            assert_eq!(cmd, "pkg_add");
            assert_eq!(args, vec!["-u", "nginx"]);
        }

        #[test]
        fn search_command() {
            let (cmd, args) = get_pkg_command("search", "nginx").unwrap();
            assert_eq!(cmd, "pkg_info");
            assert_eq!(args, vec!["-Q", "nginx"]);
        }
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use crate::syspkg::{detect_linux_pkg_manager, get_pkg_command};

        #[test]
        fn detects_some_manager() {
            if let Some(bin) = detect_linux_pkg_manager() {
                assert!(["apt-get", "dnf", "yum", "zypper", "pacman", "apk"].contains(&bin));
            }
        }

        #[test]
        fn update_has_no_name_arg() {
            if let Ok((_cmd, args)) = get_pkg_command("update", "irrelevant") {
                assert!(!args.join(" ").contains("irrelevant"));
            }
        }

        #[test]
        fn install_includes_name() {
            if let Ok((_cmd, args)) = get_pkg_command("install", "htop") {
                assert!(args.join(" ").contains("htop"));
            }
        }

        #[test]
        fn upgrade_has_no_name_arg() {
            if let Ok((_cmd, args)) = get_pkg_command("upgrade", "irrelevant") {
                assert!(!args.join(" ").contains("irrelevant"));
            }
        }

        #[test]
        fn unknown_operation_is_error() {
            assert!(get_pkg_command("nuke", "foo").is_err());
        }
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use crate::syspkg::get_pkg_command;

        #[test]
        fn install_command_when_brew_present() {
            if let Ok((cmd, args)) = get_pkg_command("install", "nginx") {
                assert_eq!(cmd, "brew");
                assert_eq!(args, vec!["install", "nginx"]);
            }
        }
    }
}
