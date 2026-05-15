#[cfg(test)]
mod tests {
    use crate::netipfw;

    #[test]
    fn detects_some_backend() {
        let b = netipfw::detect_backend();
        if let Some(b) = b {
            assert!(["pf", "ipfw", "nftables", "iptables"].contains(&b));
        }
    }

    #[test]
    fn rule_translation_deny_port() {
        let args = serde_json::json!({"action": "deny", "port": "22"});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(!rule.is_empty(), "empty rule for {backend}");
        }
    }

    #[test]
    fn rule_translation_allow_port_range() {
        let args = serde_json::json!({"action": "allow", "port-range": "8000-8080"});
        let rule = netipfw::translate_rule("pf", &args).unwrap();
        assert!(rule.contains("8000-8080"));
    }

    #[test]
    fn rule_translation_with_source() {
        let args = serde_json::json!({"action": "allow", "source": "10.0.0.0/8", "port": "5432"});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(rule.contains("10.0.0.0/8"), "missing source in {backend}: {rule}");
        }
    }

    #[test]
    fn rule_translation_icmp() {
        let args = serde_json::json!({"action": "deny", "protocol": "icmp"});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(rule.contains("icmp"), "missing icmp in {backend}: {rule}");
        }
    }

    #[test]
    fn rule_translation_outbound() {
        let args = serde_json::json!({"action": "deny", "direction": "out", "destination": "203.0.113.5"});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(!rule.is_empty());
        }
    }

    #[test]
    fn rule_translation_stateful() {
        let args = serde_json::json!({"action": "allow", "port": "443", "stateful": true});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(!rule.is_empty(), "empty rule for {backend}");
        }
    }

    #[test]
    fn rule_translation_log_flag() {
        let args = serde_json::json!({"action": "deny", "port": "22", "log": true});
        for backend in &["pf", "ipfw", "nftables", "iptables"] {
            let rule = netipfw::translate_rule(backend, &args).unwrap();
            assert!(!rule.is_empty(), "empty rule for {backend}");
        }
    }

    #[test]
    fn rule_translation_with_interface() {
        let args = serde_json::json!({"action": "allow", "interface": "eth0", "port": "80"});
        let pf_rule = netipfw::translate_rule("pf", &args).unwrap();
        assert!(pf_rule.contains("eth0"));
        let ipt_rule = netipfw::translate_rule("iptables", &args).unwrap();
        assert!(ipt_rule.contains("eth0"));
    }

    #[test]
    fn rule_translation_native_override() {
        let args = serde_json::json!({
            "native": "{\"pf\": \"block all\"}",
            "action": "this should be ignored"
        });
        // When native is present in parse_rule_args, it takes precedence.
        // The raw native is passed as a string; the module decodes it.
        let rule = netipfw::translate_rule("pf", &args).unwrap();
        // Without native override the common rule would be "pass ...",
        // but the native field in parse_rule_args just provides the string.
        // The actual native handling is in run(), not translate_rule.
        // This tests that translate_rule does NOT crash with extra fields.
        assert!(!rule.is_empty());
    }

    #[test]
    fn unsupported_backend_returns_error() {
        assert!(netipfw::translate_rule("foobar", &serde_json::json!({"action": "allow"})).is_err());
    }

    #[test]
    fn rule_translation_tcp_default() {
        let args = serde_json::json!({"action": "allow", "port": "443"});
        let rule = netipfw::translate_rule("pf", &args).unwrap();
        assert!(rule.contains("tcp"), "should default to tcp: {rule}");
    }

    #[test]
    fn rule_translation_direction_in_default() {
        let args = serde_json::json!({"action": "deny", "port": "25"});
        let rule = netipfw::translate_rule("pf", &args).unwrap();
        assert!(rule.contains(" in "), "should default to in: {rule}");
    }
}
