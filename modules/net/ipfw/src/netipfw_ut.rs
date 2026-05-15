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
}
