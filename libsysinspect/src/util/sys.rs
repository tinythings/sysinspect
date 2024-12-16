use nix::libc;
use std::io;
use std::{fs::read_to_string, path::PathBuf};

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
