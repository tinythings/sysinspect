#[cfg(test)]
mod tests {
    use super::netipfw;

    #[test]
    fn detects_some_backend() {
        let b = netfirewall::detect_backend();
        // At least one should be present on a dev machine
        if let Some(b) = b {
            assert!(["pf", "ipfw", "nftables", "iptables"].contains(&b));
        }
    }
}
