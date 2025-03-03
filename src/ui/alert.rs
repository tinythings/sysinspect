use super::{SysInspectUX, elements::AlertResult};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

static YES_LABEL: &str = "Yes";
static NO_LABEL: &str = "No";
static DEFAULT_BUTTON_WIDTH: u16 = 12;

impl SysInspectUX {
    pub fn dialog_purge(&self, parent: Rect, buf: &mut Buffer) {
        if !self.purge_alert_visible {
            return;
        }
        Self::yesno_popup(
            parent,
            buf,
            "Delete everything?",
            "Are you sure you want\nto delete everything?\n\nThis operation is irreversible.",
            None,
            self.purge_alert_choice.clone(),
        );
    }

    pub fn dialog_exit(&self, parent: Rect, buf: &mut Buffer) {
        if !self.exit_alert_visible {
            return;
        }

        Self::yesno_popup(parent, buf, "Quit?", "Quit the UI?", Some(Color::Blue), self.exit_alert_choice.clone());
    }

    /// Draws a button in MS-DOS style (no shadow)
    fn format_button(label: &str) -> String {
        let trimmed: String = if label.chars().count() > 10 { label.chars().take(10).collect() } else { label.to_string() };
        let total_pad = 10 - trimmed.chars().count();
        let left_pad = total_pad / 2;
        format!("[{}{}{}]", " ".repeat(left_pad), trimmed, " ".repeat(total_pad - left_pad))
    }

    /// Draws yes/no popup area
    fn yesno_popup(parent: Rect, buf: &mut Buffer, title: &str, text: &str, background: Option<Color>, choice: AlertResult) {
        let background = background.unwrap_or(Color::Red);

        let text = format!("\n{}", text);
        let text_lines = Self::get_text_lines(&text);
        let height = text_lines + 3;
        let width = (parent.width / 4).max(20);
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .title(format!(" {title} "))
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

        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bg(background))
            .render(text_area, buf);

        let yes_label = Self::format_button(YES_LABEL);
        let no_label = Self::format_button(NO_LABEL);

        let button_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((width - (yes_label.len() as u16 + 3 + no_label.len() as u16)) / 2),
                Constraint::Length(yes_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                Constraint::Length(3),
                Constraint::Length(no_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
            ])
            .split(button_area);

        let action_style = if choice != AlertResult::Default {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(background)
        };
        let cancel_style = if choice == AlertResult::Default {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(background)
        };

        Paragraph::new(yes_label).style(action_style).render(button_splits[1], buf);
        Paragraph::new(no_label).style(cancel_style).render(button_splits[3], buf);

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
