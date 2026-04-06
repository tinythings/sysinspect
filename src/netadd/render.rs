use crate::netadd::types::{AddOutcome, AddStatus, HostOp};
use colored::Colorize;
use libsysinspect::util::pad_visible;

fn status_cell(status: AddStatus) -> String {
    match status {
        AddStatus::Online | AddStatus::Removed => status.label().bright_green().bold().to_string(),
        AddStatus::Failed => status.label().bright_red().bold().to_string(),
        AddStatus::Absent => status.label().yellow().bold().to_string(),
        AddStatus::Pending => status.label().to_string(),
    }
}

fn host_cell(host: &str, status: AddStatus) -> String {
    match status {
        AddStatus::Online | AddStatus::Removed => host.bright_green().to_string(),
        AddStatus::Failed | AddStatus::Absent | AddStatus::Pending => host.red().to_string(),
    }
}

/// Render structured onboarding outcomes.
pub(crate) fn render_outcomes(rows: &[AddOutcome], op: HostOp) -> String {
    let widths = (
        rows.iter().map(|row| row.host.host.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| row.host.user.chars().count()).max().unwrap_or(4).max("USER".chars().count()),
        rows.iter().map(|row| row.display_path.chars().count()).max().unwrap_or(4).max("PATH".chars().count()),
        rows.iter().map(|row| row.platform.chars().count()).max().unwrap_or(7).max("OS/ARCH".chars().count()),
        rows.iter().map(|row| row.status.label().chars().count()).max().unwrap_or(6).max("status".chars().count()),
    );
    let mut out = vec![
        format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&"HOST".yellow().to_string(), widths.0),
            pad_visible(&"USER".yellow().to_string(), widths.1),
            pad_visible(&"PATH".yellow().to_string(), widths.2),
            pad_visible(&"OS/ARCH".yellow().to_string(), widths.3),
            pad_visible(&"status".yellow().to_string(), widths.4),
        ),
        format!("{}  {}  {}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2), "─".repeat(widths.3), "─".repeat(widths.4)),
    ];

    for row in rows {
        out.push(format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&host_cell(&row.host.host, row.status), widths.0),
            pad_visible(&row.host.user.bright_blue().to_string(), widths.1),
            pad_visible(&row.display_path, widths.2),
            pad_visible(&row.platform, widths.3),
            pad_visible(&status_cell(row.status), widths.4),
        ));
    }

    out.push(String::new());
    out.push(format!("{} for {} host{}", op.summary_label(), rows.len(), if rows.len() == 1 { "" } else { "s" }));
    out.join("\n")
}
