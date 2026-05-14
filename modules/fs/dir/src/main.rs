#![no_std]
#![no_main]

extern crate libc;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { libc::exit(1) };
}

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut stdin_buf = [0u8; 512];
    let n = unsafe { libc::read(0, stdin_buf.as_mut_ptr() as *mut libc::c_void, stdin_buf.len()) };
    if n <= 0 {
        unsafe { libc::exit(1) };
    }
    let input = &stdin_buf[..n as usize];
    let mut out = [0u8; 4096];
    let pos;

    if has_opt(input, b"help") || has_opt(input, b"\"help\"") {
        let help = include_bytes!(concat!(env!("OUT_DIR"), "/help.txt"));
        unsafe { libc::write(1, help.as_ptr() as *const libc::c_void, help.len()) };
        return 0;
    }
    if has_opt(input, b"check") {
        pos = check_dir(input, &mut out);
    } else if has_opt(input, b"present") {
        pos = ensure_present(input, &mut out);
    } else if has_opt(input, b"absent") {
        pos = ensure_absent(input, &mut out);
    } else {
        let msg = b"{\"retcode\":1,\"message\":\"No operation. Use --check, --present, or --absent\"}\n";
        let len = msg.len();
        out[..len].copy_from_slice(msg);
        pos = len;
    }

    unsafe { libc::write(1, out.as_ptr() as *const libc::c_void, pos) };
    0
}

fn has_opt(input: &[u8], name: &[u8]) -> bool {
    input.windows(name.len()).any(|w| w == name)
}

fn get_arg<'a>(input: &'a [u8], name: &[u8]) -> &'a [u8] {
    let mut i = 0usize;
    while i + name.len() + 3 < input.len() {
        if input[i] == b'"' && input[i + 1..].starts_with(name) && input[i + 1 + name.len()] == b'"' {
            let vs = i + name.len() + 4;
            let mut ve = vs;
            while ve < input.len() && input[ve] != b'"' {
                ve += 1;
            }
            return &input[vs..ve];
        }
        i += 1;
    }
    b""
}

fn parse_u32(bytes: &[u8]) -> u32 {
    let mut n = 0u32;
    for &b in bytes {
        if b.is_ascii_digit() {
            n = n * 10 + (b - b'0') as u32;
        }
    }
    n
}

fn cstr(bytes: &[u8]) -> [u8; 512] {
    let mut buf = [0u8; 512];
    let len = buf.len().min(bytes.len());
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}

fn check_dir(input: &[u8], out: &mut [u8; 4096]) -> usize {
    let name = get_arg(input, b"name");
    if name.is_empty() {
        return write_json(out, 1, b"Argument \"name\" is required", b"");
    }

    let path = cstr(name);
    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    let exists = unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) == 0 };

    let mut data_buf = [0u8; 4096];
    let mut dp = 0usize;
    dp += wb(&mut data_buf, dp, b"\"name\":\"");
    dp += wb(&mut data_buf, dp, name);
    dp += wb(&mut data_buf, dp, b"\",\"exists\":");
    dp += wb(&mut data_buf, dp, if exists { b"true" } else { b"false" });
    if exists {
        dp += wb(&mut data_buf, dp, b",\"is_dir\":");
        dp += wb(&mut data_buf, dp, if (stat.st_mode & libc::S_IFDIR as u32) != 0 { b"true" } else { b"false" });
        dp += wb(&mut data_buf, dp, b",\"mode\":\"");
        let mode = &mut [0u8; 16];
        let mode_str = format_mode(stat.st_mode & 0o777, mode);
        dp += wb(&mut data_buf, dp, mode_str);
        dp += wb(&mut data_buf, dp, b"\",\"uid\":");
        dp += wb_u32(&mut data_buf, dp, stat.st_uid);
        dp += wb(&mut data_buf, dp, b",\"gid\":");
        dp += wb_u32(&mut data_buf, dp, stat.st_gid);
    }

    write_json(out, 0, b"", &data_buf[..dp])
}

fn ensure_present(input: &[u8], out: &mut [u8; 4096]) -> usize {
    let name = get_arg(input, b"name");
    if name.is_empty() {
        return write_json(out, 1, b"Argument \"name\" is required", b"");
    }

    let mode_arg = get_arg(input, b"mode");
    let mode = if mode_arg.len() >= 3 { parse_octal(mode_arg) } else { 0o755 };
    let uid = parse_u32(get_arg(input, b"uid"));
    let gid = parse_u32(get_arg(input, b"gid"));
    let dry_run = has_opt(input, b"dry-run");

    let path = cstr(name);
    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    let exists = unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) == 0 };

    if dry_run {
        let mut data = [0u8; 4096];
        let dp;
        if exists {
            dp = wb_str(&mut data, 0, b"already exists");
        } else {
            dp = wb_str(&mut data, 0, b"would create");
        }
        return write_json(out, 0, b"[dry-run]", &data[..dp]);
    }

    if exists {
        let is_dir = (stat.st_mode & libc::S_IFDIR as u32) != 0;
        if !is_dir {
            return write_json(out, 1, b"Path exists but is not a directory", b"");
        }
        let mode_ok = (stat.st_mode & 0o777) == mode;
        let owner_ok = (uid == 0 || stat.st_uid == uid) && (gid == 0 || stat.st_gid == gid);
        if mode_ok && owner_ok {
            return write_json(out, 0, b"Directory already exists with matching attributes", b"");
        }
        if uid > 0 || gid > 0 {
            unsafe { libc::chown(path.as_ptr() as *const libc::c_char, uid, gid) };
        }
        if (stat.st_mode & 0o777) != mode {
            unsafe { libc::chmod(path.as_ptr() as *const libc::c_char, mode) };
        }
        return write_json(out, 0, b"Directory attributes updated", b"");
    }

    let ret = unsafe { libc::mkdir(path.as_ptr() as *const libc::c_char, mode) };
    if ret != 0 {
        return write_json(out, 1, b"Failed to create directory", b"");
    }
    if uid > 0 || gid > 0 {
        unsafe { libc::chown(path.as_ptr() as *const libc::c_char, uid, gid) };
    }
    write_json(out, 0, b"Directory created", b"")
}

fn ensure_absent(input: &[u8], out: &mut [u8; 4096]) -> usize {
    let name = get_arg(input, b"name");
    if name.is_empty() {
        return write_json(out, 1, b"Argument \"name\" is required", b"");
    }

    let dry_run = has_opt(input, b"dry-run");
    let path = cstr(name);
    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    let exists = unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) == 0 };

    if !exists {
        return write_json(out, 0, b"Directory does not exist", b"");
    }
    if dry_run {
        return write_json(out, 0, b"[dry-run] would remove directory", b"");
    }

    let ret = unsafe { libc::rmdir(path.as_ptr() as *const libc::c_char) };
    if ret != 0 {
        return write_json(out, 1, b"Failed to remove directory", b"");
    }
    write_json(out, 0, b"Directory removed", b"")
}

fn parse_octal(bytes: &[u8]) -> u32 {
    let mut n = 0u32;
    for &b in bytes {
        if b >= b'0' && b <= b'7' {
            n = n * 8 + (b - b'0') as u32;
        }
    }
    n
}

fn format_mode(mode: u32, buf: &mut [u8; 16]) -> &[u8] {
    let s = &mut [0u8; 5];
    s[0] = b'0';
    s[1] = b'0' + ((mode >> 6) & 7) as u8;
    s[2] = b'0' + ((mode >> 3) & 7) as u8;
    s[3] = b'0' + (mode & 7) as u8;
    buf[..4].copy_from_slice(&s[..4]);
    &buf[..4]
}

fn write_json(out: &mut [u8; 4096], retcode: i32, msg: &[u8], data: &[u8]) -> usize {
    let mut p = 0usize;
    p += wb(out, p, b"{\"retcode\":");
    p += wb_u32(out, p, retcode as u32);
    if !msg.is_empty() {
        p += wb(out, p, b",\"message\":\"");
        p += wb(out, p, msg);
        p += wb(out, p, b"\"");
    }
    if !data.is_empty() {
        p += wb(out, p, b",\"data\":{");
        p += wb(out, p, data);
        p += wb(out, p, b"}");
    }
    p += wb(out, p, b"}\n");
    p
}

fn wb(out: &mut [u8; 4096], pos: usize, bytes: &[u8]) -> usize {
    let n = bytes.len();
    if pos + n < out.len() {
        out[pos..pos + n].copy_from_slice(bytes);
        n
    } else {
        0
    }
}

fn wb_str(out: &mut [u8; 4096], pos: usize, s: &[u8]) -> usize {
    wb(out, pos, s)
}

fn wb_u32(out: &mut [u8; 4096], pos: usize, n: u32) -> usize {
    let mut buf = [0u8; 16];
    let mut i = 16;
    if n == 0 {
        out[pos] = b'0';
        return 1;
    }
    let mut v = n;
    while v > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let len = 16 - i;
    out[pos..pos + len].copy_from_slice(&buf[i..]);
    len
}
