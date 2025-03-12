use nix::libc::{self, AF_UNSPEC, AI_CANONNAME, addrinfo, freeaddrinfo, getaddrinfo};
use std::ffi::{CStr, CString};
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

/// Resolve a hostname to a FQDN
pub fn to_fqdn(hostname: &str) -> Option<String> {
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

    let fqdn = unsafe {
        if !(*res).ai_canonname.is_null() {
            Some(CStr::from_ptr((*res).ai_canonname).to_string_lossy().into_owned())
        } else {
            None
        }
    };

    unsafe {
        freeaddrinfo(res);
    }

    fqdn
}
