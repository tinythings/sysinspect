/// OS distribution and name detection.
///
/// Reads `/etc/os-release` on platforms that support it (Linux, FreeBSD, etc.)
/// and falls back to compile-time constants on others.
use super::current_os_type;

/// Map a canonical OS type to its family label.
fn os_family(os: &str) -> &str {
    match os {
        "linux" | "android" => "Linux",
        "freebsd" | "netbsd" | "openbsd" | "dragonfly" => "BSD",
        "macos" | "ios" => "Darwin",
        "solaris" | "illumos" => "SunOS",
        _ => os,
    }
}

/// Parse `/etc/os-release` and return the value for a given key, or `None`.
fn os_release_value(key: &str) -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=')
            && k == key
        {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

/// OS family name, e.g. "Linux", "BSD", "Darwin", "SunOS".
pub fn os_name() -> String {
    os_family(current_os_type()).to_string()
}

/// Distribution ID, e.g. "ubuntu", "freebsd", "dragonfly".
/// Falls back to "unknown" if `/etc/os-release` is unavailable.
pub fn os_distribution() -> String {
    os_release_value("ID").unwrap_or_else(|| "unknown".to_string())
}
