use crate::minion::setup_master_addr;
use libsysinspect::cfg::mmconf::DEFAULT_PORT;

#[test]
fn parses_ipv4_master_addr_with_port() {
    assert_eq!(setup_master_addr(Some("192.168.122.10:4207"), None).unwrap(), ("192.168.122.10".to_string(), 4207));
}

#[test]
fn parses_bracketed_ipv6_master_addr_with_port() {
    assert_eq!(setup_master_addr(Some("[2001:db8::1]:4210"), None).unwrap(), ("2001:db8::1".to_string(), 4210));
}

#[test]
fn keeps_default_port_when_none_is_given() {
    assert_eq!(setup_master_addr(Some("10.0.0.5"), None).unwrap(), ("10.0.0.5".to_string(), DEFAULT_PORT));
}

#[test]
fn falls_back_to_ssh_client_ip() {
    assert_eq!(setup_master_addr(None, Some("10.2.3.4".to_string())).unwrap(), ("10.2.3.4".to_string(), DEFAULT_PORT));
}
