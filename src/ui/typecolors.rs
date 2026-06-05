use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use super::palette;

/// Ingests a string value, detects its type, and returns a coloured `Span`.
///
/// - `"true"`  → `✓`  in `SUCCESS`
/// - `"false"` → `✗`  in `WARNING`
/// - numeric   → raw digits in `WARNING_GLOW` bold
/// - other     → as-is, no special colour
pub fn format_typed_value(raw: &str) -> Span<'static> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Span::raw(raw.to_string());
    }

    let unquoted = trimmed.trim_matches('"');

    if unquoted.eq_ignore_ascii_case("true") {
        Span::styled("\u{2714}", Style::default().fg(palette::SUCCESS))
    } else if unquoted.eq_ignore_ascii_case("false") {
        Span::styled("\u{2716}", Style::default().fg(palette::WARNING))
    } else if is_numeric(unquoted) {
        Span::styled(unquoted.to_string(), Style::default().fg(palette::WARNING_GLOW).add_modifier(Modifier::BOLD))
    } else {
        Span::raw(raw.to_string())
    }
}

fn is_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.trim_start_matches(['+', '-']);
    if s.is_empty() {
        return false;
    }
    let s = if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        rest
    } else if let Some(rest) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        rest
    } else if let Some(rest) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        rest
    } else {
        s
    };
    let has_dot = s.contains('.');
    let has_e = s.contains('e') || s.contains('E');
    if has_e && !has_dot {
        let parts: Vec<&str> = s.splitn(2, ['e', 'E']).collect();
        parts.len() == 2
            && !parts[0].is_empty()
            && !parts[1].is_empty()
            && parts[0].chars().all(|c| c.is_ascii_digit() || c == '.')
            && parts[1].trim_start_matches(['+', '-']).chars().all(|c| c.is_ascii_digit())
    } else {
        s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '_') && !s.is_empty() && s != "."
    }
}
