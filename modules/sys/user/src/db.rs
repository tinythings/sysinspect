use std::fs;
use std::io::Write;

/// A single passwd entry: name, uid, gid, gecos, home, shell.
#[derive(Debug, Clone)]
pub(crate) struct PasswdEntry {
    pub(crate) name: String,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) gecos: String,
    pub(crate) home: String,
    pub(crate) shell: String,
}

/// A single group entry: name, gid, members.
#[derive(Debug, Clone)]
pub(crate) struct GroupEntry {
    pub(crate) name: String,
    pub(crate) gid: u32,
    pub(crate) members: Vec<String>,
}

/// Parse a passwd line into structured fields.
pub(crate) fn parse_passwd(line: &str) -> Option<PasswdEntry> {
    let f: Vec<&str> = line.splitn(7, ':').collect();
    if f.len() < 7 || f[0].is_empty() {
        return None;
    }
    Some(PasswdEntry {
        name: f[0].to_string(),
        uid: f[2].parse().unwrap_or(0),
        gid: f[3].parse().unwrap_or(0),
        gecos: f[4].to_string(),
        home: f[5].to_string(),
        shell: f[6].to_string(),
    })
}

/// Format a passwd entry back to a colon-separated line.
pub(crate) fn format_passwd(e: &PasswdEntry) -> String {
    format!("{}:x:{}:{}:{}:{}:{}", e.name, e.uid, e.gid, e.gecos, e.home, e.shell)
}

/// Parse a group line into structured fields.
pub(crate) fn parse_group(line: &str) -> Option<GroupEntry> {
    let f: Vec<&str> = line.splitn(4, ':').collect();
    if f.len() < 4 || f[0].is_empty() {
        return None;
    }
    let members = if f[3].is_empty() { Vec::new() } else { f[3].split(',').map(|s| s.to_string()).collect() };
    Some(GroupEntry { name: f[0].to_string(), gid: f[2].parse().unwrap_or(0), members })
}

/// Format a group entry back to a colon-separated line.
pub(crate) fn format_group(e: &GroupEntry) -> String {
    format!("{}:x:{}:{}", e.name, e.gid, e.members.join(","))
}

/// Read all entries from a colon-delimited file (passwd or group).
pub(crate) fn read_db(path: &str) -> Vec<String> {
    fs::read_to_string(path).map(|s| s.lines().map(|l| l.to_string()).collect()).unwrap_or_default()
}

/// Write lines to a file atomically: write to temp, then rename.
pub(crate) fn write_db(path: &str, lines: &[String]) -> Result<(), String> {
    let tmp = format!("{path}.tmp");
    let mut f = fs::File::create(&tmp).map_err(|e| format!("Cannot create {tmp}: {e}"))?;
    for line in lines {
        writeln!(f, "{line}").map_err(|e| format!("Write error: {e}"))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("Rename error: {e}"))?;
    Ok(())
}

/// Find a free UID starting from the given minimum.
pub(crate) fn find_free_uid(lines: &[String], min: u32) -> u32 {
    let used: std::collections::BTreeSet<u32> = lines.iter().filter_map(|l| parse_passwd(l)).map(|e| e.uid).collect();
    let mut uid = min;
    while used.contains(&uid) {
        uid += 1;
    }
    uid
}

/// Find a free GID starting from the given minimum.
pub(crate) fn find_free_gid(lines: &[String], min: u32) -> u32 {
    let used: std::collections::BTreeSet<u32> = lines.iter().filter_map(|l| parse_group(l)).map(|e| e.gid).collect();
    let mut gid = min;
    while used.contains(&gid) {
        gid += 1;
    }
    gid
}

/// Platform path to the passwd file.
pub(crate) fn passwd_path() -> &'static str {
    "/etc/passwd"
}

/// Platform path to the group file.
pub(crate) fn group_path() -> &'static str {
    "/etc/group"
}

/// Platform path to the shadow file (or master.passwd on FreeBSD).
#[allow(dead_code)]
pub(crate) fn shadow_path() -> &'static str {
    if cfg!(target_os = "freebsd") { "/etc/master.passwd" } else { "/etc/shadow" }
}

/// Default shell for new users on this platform.
pub(crate) fn default_shell() -> &'static str {
    if cfg!(target_os = "freebsd") || cfg!(target_os = "openbsd") || cfg!(target_os = "netbsd") { "/bin/sh" } else { "/bin/bash" }
}
