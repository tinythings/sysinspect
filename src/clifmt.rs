use chrono::{DateTime, Utc};
use colored::Colorize;
use libsysinspect::{
    console::{ConsoleOnlineMinionRow, ConsolePayload, ConsoleTransportStatusRow},
    transport::TransportRotationStatus,
    util::pad_visible,
};

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

/// Render the `ConsolePayload::OnlineMinions` rows as a width-aware CLI table.
///
/// The output intentionally mirrors the existing terminal style used elsewhere
/// in the CLI: colored headers, aligned columns, green for healthy values, and
/// shortened minion ids for readability.
fn render_online_minions(rows: &[ConsoleOnlineMinionRow]) -> String {
    let widths = (
        rows.iter().map(online_host).map(|v| v.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| if row.ip.is_empty() { "unknown".len() } else { row.ip.chars().count() }).max().unwrap_or(2).max("IP".chars().count()),
        rows.iter().map(|row| shorten_middle(&row.minion_id, 4).chars().count()).max().unwrap_or(2).max("ID".chars().count()),
    );

    let mut out = vec![
        format!(
            "{}  {}  {}",
            pad_visible(&"HOST".bright_yellow().to_string(), widths.0),
            pad_visible(&"IP".bright_yellow().to_string(), widths.1),
            pad_visible(&"ID".bright_yellow().to_string(), widths.2),
        ),
        format!("{}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2)),
    ];

    for row in rows {
        let host_plain = online_host(row);
        let ip_plain = if row.ip.is_empty() { "unknown".to_string() } else { row.ip.clone() };
        let id_plain = shorten_middle(&row.minion_id, 4);
        let host = if row.alive { host_plain.bright_green().to_string() } else { host_plain.red().to_string() };
        let ip = if row.alive { ip_plain.bright_blue().to_string() } else { ip_plain.blue().to_string() };
        let id = if row.alive { id_plain.bright_green().to_string() } else { id_plain.green().to_string() };
        out.push(format!("{}  {}  {}", pad_visible(&host, widths.0), pad_visible(&ip, widths.1), pad_visible(&id, widths.2)));
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
    }
}
