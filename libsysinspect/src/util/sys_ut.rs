use crate::util::sys::preferred_host_ip;
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn preferred_host_ip_keeps_resolved_non_loopback_address() {
    assert_eq!(
        preferred_host_ip(Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121))), Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)))),
        Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121)))
    );
}

#[test]
fn preferred_host_ip_falls_back_to_interface_address() {
    assert_eq!(
        preferred_host_ip(Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121)))),
        Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121)))
    );
    assert_eq!(preferred_host_ip(None, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121)))), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 122, 121))));
}
