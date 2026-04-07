use chrono::{DateTime, Utc};
use colored::Colorize;
use libsysinspect::{
    console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow, ConsolePayload, ConsoleTransportStatusRow},
    traits::TraitSource,
    transport::TransportRotationStatus,
    util::pad_visible,
};
use serde_json::Value;
use std::{net::IpAddr, str::FromStr};

/// Shorten a display string by preserving the leading and trailing `edge`
/// characters and replacing the removed middle section with `...`.
///
/// This is used for long identifiers such as minion ids and transport key ids,
/// where operators still need enough prefix and suffix characters to visually
/// distinguish adjacent values in a table.
///
/// Returns the original string unchanged when the input is already short enough.
fn shorten_middle(value: &str, edge: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= (edge * 2) + 3 {
        return value.to_string();
    }

    let prefix: String = chars.iter().take(edge).collect();
    let suffix: String = chars[chars.len().saturating_sub(edge)..].iter().collect();
    format!("{prefix}...{suffix}")
}

/// Convert an optional timestamp into a compact relative age label.
///
/// The label is rendered relative to `now` using the shortest useful unit for
/// the console tables: seconds, minutes, hours, days, or weeks. Missing values
/// are rendered as `-`.
///
/// This keeps CLI output stable and compact while still being easy to scan in
/// both plain terminal and future TUI views.
fn relative_label(ts: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    let Some(ts) = ts else {
        return "-".to_string();
    };

    let seconds = (now - ts).num_seconds().max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }

    let days = hours / 24;
    if days < 7 {
        return format!("{days}d");
    }

    format!("{}w", days / 7)
}

/// Choose the preferred host label for an online-minion row.
///
/// The formatter prefers the fully qualified domain name when available, then
/// falls back to the short hostname, and finally to `unknown` when neither was
/// recorded by the master.
fn online_host(row: &ConsoleOnlineMinionRow) -> String {
    if !row.fqdn.trim().is_empty() {
        return row.fqdn.clone();
    }
    if !row.hostname.trim().is_empty() {
        return row.hostname.clone();
    }
    "unknown".to_string()
}

/// Choose the preferred host label for a transport-status row.
///
/// The formatter prefers the fully qualified domain name, then the short host
/// name. If no host traits were persisted, it falls back to the raw minion id
/// so the row still has a stable label.
fn transport_host(row: &ConsoleTransportStatusRow) -> String {
    if !row.fqdn.trim().is_empty() {
        return row.fqdn.clone();
    }
    if !row.hostname.trim().is_empty() {
        return row.hostname.clone();
    }
    row.minion_id.clone()
}

/// Convert a raw transport rotation state into a plain label and a colorized
/// label suitable for terminal output.
///
/// The plain label is used for width calculations so ANSI color codes do not
/// distort the table layout. The colorized label is used when emitting the
/// final rendered row.
fn rotation_label(rotation: Option<TransportRotationStatus>) -> (&'static str, String) {
    match rotation {
        Some(TransportRotationStatus::Idle) => ("Idle", "Idle".bright_green().to_string()),
        Some(TransportRotationStatus::Pending) => ("Pending", "Pending".yellow().to_string()),
        Some(TransportRotationStatus::InProgress) => ("InProgress", "InProgress".bright_yellow().to_string()),
        Some(TransportRotationStatus::RollbackReady) => ("RollbackReady", "RollbackReady".bright_blue().to_string()),
        None => ("Missing", "Missing".red().to_string()),
    }
}

fn is_mac_address(value: &str) -> bool {
    let parts = value.split([':', '-']).collect::<Vec<_>>();
    (parts.len() == 6 || parts.len() == 8) && parts.iter().all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 { format!("{} {}", bytes, UNITS[unit]) } else { format!("{value:.1} {}", UNITS[unit]) }
}

fn format_minion_info_value(key: &str, value: &Value) -> (String, String) {
    match value {
        Value::Null => ("null".to_string(), "null".bright_white().to_string()),
        Value::Bool(flag) => {
            let plain = if *flag { "yes" } else { "no" }.to_string();
            let colored = if *flag { plain.clone().bright_green().to_string() } else { plain.clone().red().to_string() };
            (plain, colored)
        }
        Value::Number(number) => {
            if (key == "hardware.memory" || key == "hardware.swap")
                && let Some(bytes) = number.as_u64()
            {
                let plain = human_size(bytes);
                return (plain.clone(), plain.bright_white().to_string());
            }

            let plain = number.to_string();
            (plain.clone(), plain.bright_white().to_string())
        }
        Value::String(text) => {
            let plain = text.clone();
            let colored = if IpAddr::from_str(text).is_ok() {
                plain.clone().bright_blue().to_string()
            } else if is_mac_address(text) {
                plain.clone().blue().to_string()
            } else {
                plain.clone().bright_green().to_string()
            };
            (plain, colored)
        }
        _ => {
            let plain = value.to_string();
            (plain.clone(), plain.bright_green().to_string())
        }
    }
}

fn format_minion_info_lines(key: &str, value: &Value) -> Vec<(String, String)> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return vec![("[]".to_string(), "[]".bright_white().to_string())];
            }

            let mut lines = Vec::new();
            for item in items {
                match item {
                    Value::Array(_) => lines.extend(format_minion_info_lines(key, item)),
                    Value::String(text) => lines.push((text.clone(), text.green().to_string())),
                    _ => {
                        let plain = item.to_string();
                        lines.push((plain.clone(), plain.bright_green().to_string()));
                    }
                }
            }
            lines
        }
        Value::Object(_) => {
            let plain = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
            plain.lines().map(|line| (line.to_string(), line.bright_green().to_string())).collect()
        }
        _ => vec![format_minion_info_value(key, value)],
    }
}

fn minion_info_key_label(key: &str, source: TraitSource) -> String {
    match source {
        TraitSource::Preset => key.bright_yellow().bold().to_string(),
        TraitSource::Static => key.bright_cyan().bold().to_string(),
        TraitSource::Function => key.bright_white().bold().to_string(),
    }
}

fn render_minion_info(rows: &[ConsoleMinionInfoRow]) -> String {
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| a.key.cmp(&b.key));

    let widths = (
        rows.iter().map(|row| row.key.chars().count()).max().unwrap_or(3).max("KEY".chars().count()),
        rows.iter()
            .flat_map(|row| format_minion_info_lines(&row.key, &row.value).into_iter().map(|(plain, _)| plain.chars().count()))
            .max()
            .unwrap_or(5)
            .max("VALUE".chars().count()),
    );

    let mut out = vec![
        format!("{}  {}", pad_visible(&"KEY".yellow().to_string(), widths.0), pad_visible(&"VALUE".yellow().to_string(), widths.1)),
        format!("{}  {}", "─".repeat(widths.0), "─".repeat(widths.1)),
    ];
    for row in rows {
        let values = format_minion_info_lines(&row.key, &row.value);
        let key_label = minion_info_key_label(&row.key, row.source);

        for (index, (_plain, colored)) in values.into_iter().enumerate() {
            if index == 0 {
                out.push(format!("{}  {}", pad_visible(&key_label, widths.0), pad_visible(&colored, widths.1)));
            } else {
                out.push(format!("{}  {}", " ".repeat(widths.0), pad_visible(&format!("  {colored}"), widths.1)));
            }
        }
    }

    out.join("\n")
}

/// Render the `ConsolePayload::OnlineMinions` rows as a width-aware CLI table.
///
/// The output intentionally mirrors the existing terminal style used elsewhere
/// in the CLI: colored headers, aligned columns, green for healthy values, and
/// shortened minion ids for readability.
fn render_online_minions(rows: &[ConsoleOnlineMinionRow]) -> String {
    let widths = (
        rows.iter().map(online_host).map(|v| v.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| if row.ip.is_empty() { "unknown".len() } else { row.ip.chars().count() }).max().unwrap_or(2).max("IP".chars().count()),
        rows.iter()
            .map(|row| {
                if row.outdated && !row.version.is_empty() && !row.target_version.is_empty() {
                    format!("{} -> {}", row.version, row.target_version).chars().count()
                } else if row.version.is_empty() {
                    1
                } else {
                    row.version.chars().count()
                }
            })
            .max()
            .unwrap_or(3)
            .max("VERSION".chars().count()),
        rows.iter().map(|row| shorten_middle(&row.minion_id, 4).chars().count()).max().unwrap_or(2).max("ID".chars().count()),
        rows.iter().map(|row| if row.alive { "online".len() } else { "offline".len() }).max().unwrap_or(6).max("STATUS".chars().count()),
    );

    let mut out = vec![
        format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&"HOST".bright_yellow().to_string(), widths.0),
            pad_visible(&"IP".bright_yellow().to_string(), widths.1),
            pad_visible(&"VERSION".bright_yellow().to_string(), widths.2),
            pad_visible(&"ID".bright_yellow().to_string(), widths.3),
            pad_visible(&"STATUS".bright_yellow().to_string(), widths.4),
        ),
        format!("{}  {}  {}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2), "─".repeat(widths.3), "─".repeat(widths.4)),
    ];

    for row in rows {
        let host_plain = online_host(row);
        let ip_plain = if row.ip.is_empty() { "unknown".to_string() } else { row.ip.clone() };
        let version_plain = if row.outdated && !row.version.is_empty() && !row.target_version.is_empty() {
            format!("{} -> {}", row.version, row.target_version)
        } else if row.version.is_empty() {
            "-".to_string()
        } else {
            row.version.clone()
        };
        let id_plain = shorten_middle(&row.minion_id, 4);
        let host = if row.alive { host_plain.bright_green().to_string() } else { host_plain.red().to_string() };
        let ip = if row.alive { ip_plain.bright_blue().to_string() } else { ip_plain.blue().to_string() };
        let version = if row.outdated {
            version_plain.bright_yellow().bold().to_string()
        } else if row.version.is_empty() {
            version_plain.red().to_string()
        } else {
            version_plain.bright_green().to_string()
        };
        let id = if row.alive { id_plain.bright_green().to_string() } else { id_plain.green().to_string() };
        let status = if row.alive {
            "online".bright_green().bold().to_string()
        } else {
            "offline".red().bold().to_string()
        };
        out.push(format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&host, widths.0),
            pad_visible(&ip, widths.1),
            pad_visible(&version, widths.2),
            pad_visible(&id, widths.3),
            pad_visible(&status, widths.4)
        ));
    }

    out.join("\n")
}

/// Render the `ConsolePayload::TransportStatus` rows as a width-aware CLI table.
///
/// This function owns all transport-status presentation concerns on the CLI
/// side: hostname selection, id shortening, relative time formatting, color
/// coding for rotation state, and aligned column layout.
///
/// The master deliberately does not perform any of this formatting anymore and
/// only supplies typed row data.
fn render_transport_status(rows: &[ConsoleTransportStatusRow]) -> String {
    let now = Utc::now();
    let widths = (
        rows.iter().map(transport_host).map(|v| v.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter()
            .map(|row| row.active_key_id.as_deref().map(|id| shorten_middle(id, 3).chars().count()).unwrap_or(1))
            .max()
            .unwrap_or(3)
            .max("KEY".chars().count()),
        rows.iter().map(|row| relative_label(row.last_rotated_at, now).chars().count()).max().unwrap_or(3).max("AGE".chars().count()),
        rows.iter().map(|row| relative_label(row.last_handshake_at, now).chars().count()).max().unwrap_or(4).max("SEEN".chars().count()),
        rows.iter().map(|row| relative_label(row.last_rotated_at, now).chars().count()).max().unwrap_or(7).max("ROTATED".chars().count()),
        rows.iter().map(|row| rotation_label(row.rotation.clone()).0.chars().count()).max().unwrap_or(8).max("ROTATION".chars().count()),
    );

    let mut out = vec![
        format!(
            "{}  {}  {}  {}  {}  {}",
            pad_visible(&"HOST".bright_yellow().to_string(), widths.0),
            pad_visible(&"KEY".bright_yellow().to_string(), widths.1),
            pad_visible(&"AGE".bright_yellow().to_string(), widths.2),
            pad_visible(&"SEEN".bright_yellow().to_string(), widths.3),
            pad_visible(&"ROTATED".bright_yellow().to_string(), widths.4),
            pad_visible(&"ROTATION".bright_yellow().to_string(), widths.5),
        ),
        format!(
            "{}  {}  {}  {}  {}  {}",
            "─".repeat(widths.0),
            "─".repeat(widths.1),
            "─".repeat(widths.2),
            "─".repeat(widths.3),
            "─".repeat(widths.4),
            "─".repeat(widths.5),
        ),
    ];

    for row in rows {
        let host = transport_host(row).bright_green().to_string();
        let key = row.active_key_id.as_deref().map(|id| shorten_middle(id, 3).green().to_string()).unwrap_or_else(|| "-".to_string());
        let age = relative_label(row.last_rotated_at, now);
        let seen = relative_label(row.last_handshake_at, now);
        let rotated = relative_label(row.last_rotated_at, now);
        let rotation = rotation_label(row.rotation.clone()).1;
        out.push(format!(
            "{}  {}  {}  {}  {}  {}",
            pad_visible(&host, widths.0),
            pad_visible(&key, widths.1),
            pad_visible(&age, widths.2),
            pad_visible(&seen, widths.3),
            pad_visible(&rotated, widths.4),
            pad_visible(&rotation, widths.5),
        ));
    }

    out.join("\n")
}

/// Render a structured console payload into the current stdout-oriented CLI
/// representation.
///
/// The returned string is intentionally display-ready for the command-line
/// client. Empty payloads and acknowledgement payloads that are not meant to be
/// shown return an empty string.
pub fn render_console_payload(payload: &ConsolePayload) -> String {
    match payload {
        ConsolePayload::Empty => String::new(),
        ConsolePayload::Text { value } => value.clone(),
        ConsolePayload::StringList { items } => items.join("\n"),
        ConsolePayload::RotationSummary { online_count, queued_count } => format!(
            "Rotation staged: {} online dispatch{}, {} pending for offline minion{}",
            online_count,
            if *online_count == 1 { "" } else { "es" },
            queued_count,
            if *queued_count == 1 { "" } else { "s" }
        ),
        ConsolePayload::Ack { action, target, count, items } => match action.as_str() {
            "create_profile" => format!("Created profile {}", target.bright_yellow()),
            "delete_profile" => format!("Deleted profile {}", target.bright_yellow()),
            "update_profile" => format!("Updated profile {}", target.bright_yellow()),
            "remove_minion" => format!("Unregistered minion {}", target.bright_yellow()),
            "apply_profiles" => {
                format!("Applied profiles {} on {} minion{}", items.join(", ").bright_yellow(), count, if *count == 1 { "" } else { "s" })
            }
            "remove_profiles" => {
                format!("Removed profiles {} on {} minion{}", items.join(", ").bright_yellow(), count, if *count == 1 { "" } else { "s" })
            }
            "accepted_console_command" => String::new(),
            _ => action.clone(),
        },
        ConsolePayload::OnlineMinions { rows } => render_online_minions(rows),
        ConsolePayload::TransportStatus { rows } => render_transport_status(rows),
        ConsolePayload::MinionInfo { rows } => render_minion_info(rows),
    }
}
