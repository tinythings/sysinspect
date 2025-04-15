use super::{SysInspectUX, elements::AlertResult};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

enum AlertButtons {
    YesNo,
    OkCancel,
    Ok,
    Quit,
}

static YES_LABEL: &str = "Yes";
static NO_LABEL: &str = "No";
static OK_LABEL: &str = "OK";
static CANCEL_LABEL: &str = "Cancel";
static QUIT_LABEL: &str = "Quit";
static DEFAULT_BUTTON_WIDTH: u16 = 12;

#[allow(clippy::too_many_arguments)]
impl SysInspectUX {
    pub fn dialog_error(&self, parent: Rect, buf: &mut Buffer) {
        if !self.error_alert_visible {
            return;
        }
        Self::quit_popup(
            parent,
            buf,
            Some("Error"),
            &format!(
                "An unexpected error occurred:\n{}\n\nPlease check the logs for more information.",
                self.error_alert_message
            ),
            Alignment::Center,
            Some(Color::Red),
            AlertResult::Quit,
            Some(0),
        );
    }

    pub fn dialog_purge(&self, parent: Rect, buf: &mut Buffer) {
        if !self.purge_alert_visible {
            return;
        }
        Self::yesno_popup(
            parent,
            buf,
            Some("Delete everything?"),
            "Are you sure you want\nto delete everything?\n\nThis operation is irreversible.",
            None,
            self.purge_alert_choice.clone(),
            None,
        );
    }

    pub fn dialog_help(&self, parent: Rect, buf: &mut Buffer) {
        if !self.help_popup_visible {
            return;
        }
        Self::quit_popup(
            parent,
            buf,
            Some("Help"),
            "\"p\" - to purge all records from the dataase\n\"q\" - to quit the UI\n\"h\" - to show this help\n",
            Alignment::Left,
            Some(Color::Green),
            AlertResult::Quit,
            None,
        );
    }

    pub fn dialog_exit(&self, parent: Rect, buf: &mut Buffer) {
        if !self.exit_alert_visible {
            return;
        }

        Self::okcancel_popup(parent, buf, None, "Quit the UI?", Some(Color::Blue), self.exit_alert_choice.clone(), None);
    }

    /// Draws a button in MS-DOS style (no shadow)
    fn format_button(label: &str) -> String {
        let trimmed: String = if label.chars().count() > 10 { label.chars().take(10).collect() } else { label.to_string() };
        let total_pad = 10 - trimmed.chars().count();
        let left_pad = total_pad / 2;
        format!("[{}{}{}]", " ".repeat(left_pad), trimmed, " ".repeat(total_pad - left_pad))
    }

    /// Draws quit popup area
    fn quit_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, text_align: Alignment, background: Option<Color>,
        choice: AlertResult, width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, text_align, choice, AlertButtons::Quit, width);
    }

    /// Draws ok/cancel popup area
    fn okcancel_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, choice: AlertResult,
        width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, Alignment::Center, choice, AlertButtons::OkCancel, width);
    }

    /// Draws yes/no popup area
    fn yesno_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, choice: AlertResult,
        width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, Alignment::Center, choice, AlertButtons::YesNo, width);
    }

    /// Draws a popup area
    fn _popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, text_align: Alignment,
        choice: AlertResult, buttons: AlertButtons, width: Option<u16>,
    ) {
        let background = background.unwrap_or(Color::Red);

        let text = format!("\n{}", text);
        let text_lines = Self::get_text_lines(&text);
        let height = text_lines + 3;

        #[allow(clippy::unnecessary_unwrap)]
        let mut width = if width.is_none() { (parent.width / 4).max(20) } else { width.unwrap() };
        if width == 0 {
            width = Self::get_max_width_lines(&text) + 2;
        }

        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .title(if let Some(t) = title { format!(" {t} ") } else { "".to_string() })
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(Color::Black).bg(Color::Gray))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray))
            .style(Style::default().bg(background));

        let popup_inner = popup_block.inner(canvas);
        popup_block.render(canvas, buf);

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(text_lines), Constraint::Length(1)])
            .split(popup_inner);

        let text_area = vertical_chunks[0];
        let button_area = vertical_chunks[1];

        Paragraph::new(text).alignment(text_align).style(Style::default().fg(Color::White).bg(background)).render(text_area, buf);
        let (lbtn_label, rbtn_label) = match buttons {
            AlertButtons::YesNo => (Self::format_button(YES_LABEL), Self::format_button(NO_LABEL)),
            AlertButtons::OkCancel => (Self::format_button(OK_LABEL), Self::format_button(CANCEL_LABEL)),
            AlertButtons::Ok => (Self::format_button(OK_LABEL), "".to_string()),
            AlertButtons::Quit => (Self::format_button(QUIT_LABEL), "".to_string()),
        };

        let button_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((width - (lbtn_label.len() as u16 + 3 + rbtn_label.len() as u16)) / 2),
                Constraint::Length(lbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                Constraint::Length(3),
                Constraint::Length(rbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
            ])
            .split(button_area);

        // Button styles
        let b_passive = if choice != AlertResult::Default {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(background)
        };
        let b_active = if choice == AlertResult::Default {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(background)
        };

        Paragraph::new(lbtn_label).style(b_passive).render(button_splits[1], buf);
        Paragraph::new(rbtn_label).style(b_active).render(button_splits[3], buf);

        // MS-DOS style shadows :-)
        for idx in 0..width {
            let x = buf.cell_mut(Position::new(x + 2 + idx, y + height)).unwrap();
            x.set_bg(Color::Black);
            x.set_fg(Color::DarkGray);
        }

        for offset in 0..2 {
            for idx in 0..height {
                let x = buf.cell_mut(Position::new(x + width + offset, y + idx + 1)).unwrap();
                x.set_bg(Color::Black);
                x.set_fg(Color::DarkGray);
            }
        }
    }
}
