use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// Check TCP or UDP port connectivity via raw std::net (libc underneath).
/// No external binaries needed — works on bare kernel+busybox.
pub fn check_connectivity(rt: &ModRequest, rsp: &mut ModResponse) {
    let host = runtime::get_arg(rt, "host");
    let port = runtime::get_arg(rt, "port");
    let proto = runtime::get_arg(rt, "protocol");
    let timeout = runtime::get_arg(rt, "timeout");

    if host.is_empty() || port.is_empty() {
        rsp.set_retcode(1);
        rsp.set_message("Arguments \"host\" and \"port\" are required for --connect");
        return;
    }

    let protocol = if proto == "udp" { "udp" } else { "tcp" };
    let timeout_secs = timeout.parse::<u64>().unwrap_or(3);
    let port_num: u16 = match port.parse() {
        Ok(p) if p > 0 => p,
        _ => {
            rsp.set_retcode(1);
            rsp.set_message(&format!("Invalid port: {port}"));
            return;
        }
    };

    let mut data = HashMap::new();
    data.insert("host".to_string(), serde_json::Value::String(host.clone()));
    data.insert("port".to_string(), serde_json::Value::Number(serde_json::Number::from(port_num)));
    data.insert("protocol".to_string(), serde_json::Value::String(protocol.to_string()));

    let start = Instant::now();
    let open = if protocol == "udp" { check_udp(&host, port_num, timeout_secs) } else { check_tcp(&host, port_num, timeout_secs) };
    let elapsed = start.elapsed().as_millis() as u64;

    data.insert("open".to_string(), serde_json::Value::Bool(open));
    data.insert("latency_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(elapsed)));

    rsp.set_retcode(if open { 0 } else { 1 });
    rsp.set_message(&format!("Port {port}/{protocol} on {host} is {} ({elapsed}ms)", if open { "open" } else { "closed or unreachable" }));

    if let Err(e) = rsp.set_data(&data) {
        rsp.add_warning(&format!("{e}"));
    }
}

fn check_tcp(host: &str, port: u16, timeout_secs: u64) -> bool {
    let addr = match format!("{host}:{port}").to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return false,
    };
    TcpStream::connect_timeout(&addr, Duration::from_secs(timeout_secs)).is_ok()
}

fn check_udp(host: &str, port: u16, timeout_secs: u64) -> bool {
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = sock.set_write_timeout(Some(Duration::from_secs(timeout_secs)));
    let addr = match format!("{host}:{port}").to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return false,
    };
    sock.send_to(b"", addr).is_ok()
}
