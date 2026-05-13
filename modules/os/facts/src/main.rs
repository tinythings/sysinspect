extern crate libc;

fn main() {
    let help = include_bytes!(concat!(env!("OUT_DIR"), "/help.txt"));
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        unsafe { libc::write(1, help.as_ptr() as *const libc::c_void, help.len()) };
        return;
    }

    let mut stdin_buf = [0u8; 512];
    let n = unsafe { libc::read(0, stdin_buf.as_mut_ptr() as *mut libc::c_void, stdin_buf.len()) };
    if n <= 0 {
        std::process::exit(1);
    }
    let input = &stdin_buf[..n as usize];
    let mut out_buf = [0u8; 8192];
    let pos;
    if bytes_contains(input, b"\"gather\"") {
        pos = gather_facts(&mut out_buf);
    } else if bytes_contains(input, b"\"list-keys\"") {
        pos = list_keys(&mut out_buf);
    } else if bytes_contains(input, b"\"get\"") {
        pos = get_fact(input, &mut out_buf);
    } else {
        let msg = b"{\"retcode\":1,\"message\":\"No operation. Use --gather, --get, or --list-keys\"}\n";
        out_buf[..msg.len()].copy_from_slice(msg);
        pos = msg.len();
    }
    unsafe { libc::write(1, out_buf.as_ptr() as *const libc::c_void, pos) };
}

fn bytes_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn gather_facts(out: &mut [u8; 8192]) -> usize {
    let mut p = 0usize;
    p += wb(out, p, b"{\"retcode\":0,\"data\":{");
    p += wkv(out, p, b"os", current_os().as_bytes());
    p += w1(out, p, b',');
    p += wkv(out, p, b"arch", current_arch().as_bytes());
    p += w1(out, p, b',');
    p += wke(out, p, b"hostname", hostname());
    p += w1(out, p, b',');
    p += wke(out, p, b"kernel", uname());
    p += w1(out, p, b',');
    p += wks(out, p, b"uptime_seconds", uptime());
    #[cfg(target_os = "linux")]
    {
        p += w1(out, p, b',');
        p += wks(out, p, b"memory_total_kb", meminfo(b"MemTotal"));
        p += w1(out, p, b',');
        p += wks(out, p, b"memory_free_kb", meminfo(b"MemFree"));
        p += w1(out, p, b',');
        p += wks(out, p, b"swap_total_kb", meminfo(b"SwapTotal"));
        p += w1(out, p, b',');
        p += wks(out, p, b"swap_free_kb", meminfo(b"SwapFree"));
        p += w1(out, p, b',');
        p += wke(out, p, b"cpu_model", cpu_model());
        p += w1(out, p, b',');
        p += wks(out, p, b"cpu_cores", cpu_cores());
        p += w1(out, p, b',');
        p += wks(out, p, b"load_1m", loadavg(0));
        p += w1(out, p, b',');
        p += wks(out, p, b"load_5m", loadavg(1));
    }
    p += w1(out, p, b'}');
    p += wb(out, p, b"}\n");
    p
}

fn list_keys(out: &mut [u8; 8192]) -> usize {
    let keys = b"{\"retcode\":0,\"data\":[\"os\",\"arch\",\"hostname\",\"kernel\",\"uptime_seconds\",\"memory_total_kb\",\"memory_free_kb\",\"swap_total_kb\",\"swap_free_kb\",\"cpu_model\",\"cpu_cores\",\"load_1m\",\"load_5m\"]}\n";
    let mut p = 0usize;
    p += wb(out, p, keys);
    p
}

fn get_fact(input: &[u8], out: &mut [u8; 8192]) -> usize {
    let key = find_arg(input, b"key");
    let mut p = 0usize;
    p += wb(out, p, b"{\"retcode\":0,\"data\":{");
    match key {
        Some(b"os") => p += wkv(out, p, b"os", current_os().as_bytes()),
        Some(b"arch") => p += wkv(out, p, b"arch", current_arch().as_bytes()),
        Some(b"hostname") => p += wke(out, p, b"hostname", hostname()),
        Some(b"kernel") => p += wke(out, p, b"kernel", uname()),
        _ => {}
    }
    p += w1(out, p, b'}');
    p += wb(out, p, b"}\n");
    p
}

fn find_arg<'a>(input: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let mut i = 0usize;
    while i < input.len() && !input[i..].starts_with(b"\"arguments\"") {
        i += 1;
    }
    let rest = &input[i..];
    let mut j = 0usize;
    while j + name.len() + 3 < rest.len() {
        if rest[j] == b'"' && rest[j + 1..].starts_with(name) && rest[j + 1 + name.len()] == b'"' {
            let vs = j + name.len() + 4;
            let mut ve = vs;
            while ve < rest.len() && rest[ve] != b'"' {
                ve += 1;
            }
            return Some(&rest[vs..ve]);
        }
        j += 1;
    }
    None
}

fn current_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else if cfg!(target_os = "openbsd") {
        "openbsd"
    } else if cfg!(target_os = "netbsd") {
        "netbsd"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "solaris") {
        "solaris"
    } else {
        "unknown"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else if cfg!(target_arch = "riscv64") {
        "riscv64"
    } else {
        "unknown"
    }
}

fn hostname() -> [u8; 256] {
    let mut buf = [0u8; 256];
    unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    buf
}

fn uname() -> [u8; 256] {
    let mut uts: libc::utsname = unsafe { core::mem::zeroed() };
    unsafe { libc::uname(&mut uts) };
    let mut buf = [0u8; 256];
    let src = unsafe { core::ffi::CStr::from_ptr(uts.release.as_ptr()) };
    let bytes = src.to_bytes();
    let end = buf.len().min(bytes.len());
    buf[..end].copy_from_slice(&bytes[..end]);
    buf
}

fn uptime() -> [u8; 32] {
    let mut buf = [0u8; 512];
    if let Some(n) = rf(b"/proc/uptime\0", &mut buf) {
        let s = &buf[..n];
        let sp = s.iter().position(|&b| b == b' ').unwrap_or(s.len());
        let mut out = [0u8; 32];
        let end = out.len().min(sp);
        out[..end].copy_from_slice(&s[..end]);
        return out;
    }
    [b'0'; 32]
}

fn rf(path: &[u8], buf: &mut [u8]) -> Option<usize> {
    let fd = unsafe { libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY) };
    if fd < 0 {
        return None;
    }
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
    unsafe { libc::close(fd) };
    if n > 0 { Some(n as usize) } else { None }
}

fn meminfo(key: &[u8]) -> [u8; 32] {
    let mut buf = [0u8; 2048];
    if let Some(n) = rf(b"/proc/meminfo\0", &mut buf) {
        for line in buf[..n].split(|&b| b == b'\n') {
            if line.starts_with(key) {
                let mut s = key.len();
                while s < line.len() && (line[s] == b' ' || line[s] == b':' || line[s] == b'\t') {
                    s += 1;
                }
                let mut e = s;
                while e < line.len() && line[e].is_ascii_digit() {
                    e += 1;
                }
                let mut out = [0u8; 32];
                let l = out.len().min(e - s);
                out[..l].copy_from_slice(&line[s..s + l]);
                return out;
            }
        }
    }
    [b'0'; 32]
}

fn cpu_model() -> [u8; 256] {
    let mut buf = [0u8; 4096];
    if let Some(n) = rf(b"/proc/cpuinfo\0", &mut buf) {
        for line in buf[..n].split(|&b| b == b'\n') {
            if line.starts_with(b"model name") {
                let c = line.iter().position(|&b| b == b':').unwrap_or(0);
                let mut s = c + 1;
                while s < line.len() && line[s].is_ascii_whitespace() {
                    s += 1;
                }
                let mut out = [0u8; 256];
                let l = out.len().min(line.len() - s);
                out[..l].copy_from_slice(&line[s..s + l]);
                return out;
            }
        }
    }
    [0u8; 256]
}

fn cpu_cores() -> [u8; 32] {
    let mut buf = [0u8; 4096];
    if let Some(n) = rf(b"/proc/cpuinfo\0", &mut buf) {
        let count = buf[..n].split(|&b| b == b'\n').filter(|l| l.starts_with(b"processor")).count();
        return itoa(count);
    }
    [b'0'; 32]
}

fn loadavg(idx: usize) -> [u8; 32] {
    let mut buf = [0u8; 256];
    if let Some(n) = rf(b"/proc/loadavg\0", &mut buf) {
        let mut field = 0usize;
        let mut s = 0usize;
        for (i, &b) in buf[..n].iter().enumerate() {
            if b == b' ' {
                if field == idx {
                    let mut out = [0u8; 32];
                    let l = out.len().min(i - s);
                    out[..l].copy_from_slice(&buf[s..s + l]);
                    return out;
                }
                field += 1;
                s = i + 1;
            }
        }
    }
    [b'0'; 32]
}

fn itoa(mut n: usize) -> [u8; 32] {
    let mut tmp = [0u8; 32];
    let mut i = 31;
    if n == 0 {
        tmp[30] = b'0';
        return tmp;
    }
    while n > 0 && i > 0 {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    let mut out = [0u8; 32];
    let l = 32 - i;
    out[..l].copy_from_slice(&tmp[i..]);
    out
}

fn wb(out: &mut [u8; 8192], pos: usize, bytes: &[u8]) -> usize {
    let n = bytes.len();
    if pos + n < out.len() {
        out[pos..pos + n].copy_from_slice(bytes);
        n
    } else {
        0
    }
}

fn w1(out: &mut [u8; 8192], pos: usize, b: u8) -> usize {
    if pos < out.len() {
        out[pos] = b;
        1
    } else {
        0
    }
}

fn wkv(out: &mut [u8; 8192], pos: usize, key: &[u8], val: &[u8]) -> usize {
    let mut n = 0usize;
    n += w1(out, pos + n, b'"');
    n += wb(out, pos + n, key);
    n += wb(out, pos + n, b"\":\"");
    n += wb(out, pos + n, val);
    n += w1(out, pos + n, b'"');
    n
}

fn wke(out: &mut [u8; 8192], pos: usize, key: &[u8], val: [u8; 256]) -> usize {
    let len = val.iter().position(|&b| b == 0).unwrap_or(val.len());
    let mut n = 0usize;
    n += w1(out, pos + n, b'"');
    n += wb(out, pos + n, key);
    n += wb(out, pos + n, b"\":\"");
    n += wb(out, pos + n, &val[..len]);
    n += w1(out, pos + n, b'"');
    n
}

fn wks(out: &mut [u8; 8192], pos: usize, key: &[u8], val: [u8; 32]) -> usize {
    let len = val.iter().position(|&b| b == 0).unwrap_or(val.len());
    let mut n = 0usize;
    n += w1(out, pos + n, b'"');
    n += wb(out, pos + n, key);
    n += wb(out, pos + n, b"\":\"");
    n += wb(out, pos + n, &val[..len]);
    n += w1(out, pos + n, b'"');
    n
}
