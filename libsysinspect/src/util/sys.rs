use if_addrs::get_if_addrs;
use nix::libc::{self, AF_UNSPEC, AI_CANONNAME, addrinfo, freeaddrinfo, getaddrinfo};
use std::ffi::{CStr, CString};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::{fs::read_to_string, path::PathBuf};
use std::{io, ptr};

/// Kills a proces from a PID file
pub fn kill_process(pidf: PathBuf, wait: Option<u64>) -> io::Result<()> {
    let pid_str = read_to_string(pidf)?;
    let pid = pid_str
        .trim()
        .parse::<i32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid PID file content: {}", e)))?;

    unsafe {
        if libc::kill(pid, libc::SIGTERM) != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    if let Some(wait) = wait {
        std::thread::sleep(std::time::Duration::from_secs(wait));
    }

    // Again! >:-[=]
    unsafe {
        if libc::kill(pid, 0) == 0 && libc::kill(pid, libc::SIGKILL) != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

/// Resolves the given hostname to its canonical FQDN and a default outer IP address.
pub fn to_fqdn_ip(hostname: &str) -> Option<(String, IpAddr)> {
    let hostname = CString::new(hostname).ok()?;
    let hints = addrinfo {
        ai_flags: AI_CANONNAME,
        ai_family: AF_UNSPEC,
        ai_socktype: 0,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_addr: ptr::null_mut(),
        ai_canonname: ptr::null_mut(),
        ai_next: ptr::null_mut(),
    };

    let mut res: *mut addrinfo = ptr::null_mut();
    if unsafe { getaddrinfo(hostname.as_ptr(), ptr::null(), &hints, &mut res) } != 0 || res.is_null() {
        return None;
    }

    let mut fqdn: Option<String> = None;
    let mut ipaddr: Option<IpAddr> = None;
    let mut curr = res;
    unsafe {
        while !curr.is_null() {
            if fqdn.is_none() && !(*curr).ai_canonname.is_null() {
                fqdn = Some(CStr::from_ptr((*curr).ai_canonname).to_string_lossy().into_owned());
            }
            if ipaddr.is_none() {
                ipaddr = extract_ip((*curr).ai_addr);
            }
            if fqdn.is_some() && ipaddr.is_some() {
                break;
            }
            curr = (*curr).ai_next;
        }
    }
    unsafe {
        freeaddrinfo(res);
    }

    if let Some(ip) = ipaddr {
        if ip.is_loopback() {
            ipaddr = ext_ipaddr();
        }
    }

    match (fqdn, ipaddr) {
        (Some(fqdn), Some(ip)) => Some((fqdn, ip)),
        _ => None,
    }
}

/// Enumerate network interfaces to find the first non-loopback IP address.
fn ext_ipaddr() -> Option<IpAddr> {
    if let Ok(interfaces) = get_if_addrs() {
        for iface in interfaces {
            if !iface.is_loopback() {
                return Some(iface.ip());
            }
        }
    }
    None
}

/// Extract an IP address (IPv4 or IPv6) from a raw sockaddr pointer.
fn extract_ip(addr: *const libc::sockaddr) -> Option<IpAddr> {
    if addr.is_null() {
        return None;
    }
    unsafe {
        match (*addr).sa_family as i32 {
            libc::AF_INET => {
                let s4: &libc::sockaddr_in = &*(addr as *const libc::sockaddr_in);
                Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(s4.sin_addr.s_addr))))
            }
            libc::AF_INET6 => {
                let s6: &libc::sockaddr_in6 = &*(addr as *const libc::sockaddr_in6);
                Some(IpAddr::V6(Ipv6Addr::from(s6.sin6_addr.s6_addr)))
            }
            _ => None,
        }
    }
}
