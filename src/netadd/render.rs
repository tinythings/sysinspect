use crate::netadd::types::AddOutcome;
use colored::Colorize;
use libsysinspect::util::pad_visible;

/// Render structured onboarding outcomes.
pub(crate) fn render_outcomes(rows: &[AddOutcome]) -> String {
    let widths = (
        rows.iter().map(|row| row.host.host.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| row.host.user.chars().count()).max().unwrap_or(4).max("USER".chars().count()),
        rows.iter().map(|row| row.display_path.chars().count()).max().unwrap_or(4).max("PATH".chars().count()),
        rows.iter().map(|row| row.platform.chars().count()).max().unwrap_or(7).max("OS/ARCH".chars().count()),
    );
    let mut out = vec![
        format!(
            "{}  {}  {}  {}",
            pad_visible(&"HOST".yellow().to_string(), widths.0),
            pad_visible(&"USER".yellow().to_string(), widths.1),
            pad_visible(&"PATH".yellow().to_string(), widths.2),
            pad_visible(&"OS/ARCH".yellow().to_string(), widths.3),
        ),
        format!("{}  {}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2), "─".repeat(widths.3)),
    ];

    for row in rows {
        out.push(format!(
            "{}  {}  {}  {}",
            pad_visible(&row.host.host.bright_green().to_string(), widths.0),
            pad_visible(&row.host.user.bright_blue().to_string(), widths.1),
            pad_visible(&row.display_path, widths.2),
            pad_visible(&row.platform, widths.3),
        ));
    }

    let notes = rows.iter().filter_map(|row| row.note.as_ref().map(|note| format!("{}: {}", row.host.host, note))).collect::<Vec<_>>();
    if !notes.is_empty() {
        out.push(String::new());
        out.push("Notes".yellow().to_string());
        out.extend(notes);
    }

    out.push(String::new());
    out.push(format!("Planned onboarding for {} host{}", rows.len(), if rows.len() == 1 { "" } else { "s" }));
    out.join("\n")
}
