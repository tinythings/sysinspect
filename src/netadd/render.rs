use crate::netadd::types::AddOutcome;
use colored::Colorize;
use libsysinspect::util::pad_visible;

const DEFAULT_DEST: &str = "<system>";

/// Render structured onboarding outcomes.
pub(crate) fn render_outcomes(rows: &[AddOutcome]) -> String {
    let widths = (
        rows.iter().map(|row| row.host.host.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| row.host.user.chars().count()).max().unwrap_or(4).max("USER".chars().count()),
        rows.iter().map(|row| row.host.path.as_deref().unwrap_or(DEFAULT_DEST).chars().count()).max().unwrap_or(4).max("PATH".chars().count()),
        rows.iter().map(|row| row.state.chars().count()).max().unwrap_or(5).max("STATE".chars().count()),
        rows.iter().map(|row| row.detail.chars().count()).max().unwrap_or(6).max("DETAIL".chars().count()),
    );
    let mut out = vec![
        format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&"HOST".yellow().to_string(), widths.0),
            pad_visible(&"USER".yellow().to_string(), widths.1),
            pad_visible(&"PATH".yellow().to_string(), widths.2),
            pad_visible(&"STATE".yellow().to_string(), widths.3),
            pad_visible(&"DETAIL".yellow().to_string(), widths.4),
        ),
        format!("{}  {}  {}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2), "─".repeat(widths.3), "─".repeat(widths.4)),
    ];

    for row in rows {
        out.push(format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&row.host.host.bright_green().to_string(), widths.0),
            pad_visible(&row.host.user.bright_blue().to_string(), widths.1),
            pad_visible(row.host.path.as_deref().unwrap_or(DEFAULT_DEST), widths.2),
            pad_visible(&row.state.bright_cyan().to_string(), widths.3),
            pad_visible(&row.detail, widths.4),
        ));
    }

    out.push(String::new());
    out.push(format!("Planned onboarding for {} host{}", rows.len(), if rows.len() == 1 { "" } else { "s" }));
    out.join("\n")
}
