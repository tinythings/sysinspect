use super::{SysInspectUX, elements::AlertResult, palette};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};
use ratatui_glamour::color::blend_2d;

#[derive(Default)]
enum AlertButtons {
    YesNo,
    OkCancel,
    #[default]
    Ok,
    Quit,
    Close,
}

static YES_LABEL: &str = "Yes";
static NO_LABEL: &str = "No";
static OK_LABEL: &str = "OK";
static CANCEL_LABEL: &str = "Cancel";
static QUIT_LABEL: &str = "Quit";
static CLOSE_LABEL: &str = "Close";
static DEFAULT_BUTTON_WIDTH: u16 = 12;

#[allow(clippy::too_many_arguments)]
impl SysInspectUX {
    pub fn dialog_error(&self, parent: Rect, buf: &mut Buffer) {
        if !self.error_alert_visible {
            return;
        }
        let max_w = ((parent.width * 3 / 4).max(50)) as usize;
        let wrapped_lines = wrap_text(&self.error_alert_message, max_w);
        let text = if wrapped_lines.is_empty() { "".to_string() } else { wrapped_lines.join("\n") };
        Self::_popup_ex(
            parent,
            buf,
            Some("Error"),
            &text,
            None,
            Alignment::Left,
            AlertResult::Quit,
            AlertButtons::Close,
            Some(0),
            Some(palette::ERROR_PEAK),
            None,
            None,
            Some(palette::WHITE),
            None,
            None,
            None,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
    }

    pub fn dialog_purge(&self, parent: Rect, buf: &mut Buffer) {
        if !self.purge_alert_visible {
            return;
        }
        Self::_popup_ex(
            parent,
            buf,
            Some("Delete everything?"),
            "Are you sure you want\nto delete everything?\n\nThis operation is irreversible.",
            None,
            Alignment::Center,
            self.purge_alert_choice.clone(),
            AlertButtons::YesNo,
            Some(44),
            Some(palette::ERROR_PEAK),
            None,
            None,
            Some(palette::WHITE),
            None,
            None,
            None,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
    }

    pub fn dialog_help(&self, parent: Rect, buf: &mut Buffer) {
        if !self.help_popup_visible {
            return;
        }
        Self::_popup_ex(
            parent,
            buf,
            Some("Help"),
            "\"c\" - call composer\n\"h\" - show this help\n\"o\" - registered minions popup\n\"p\" - purge all records\n\"q\" - quit the UI\n",
            None,
            Alignment::Left,
            AlertResult::Close,
            AlertButtons::Close,
            Some(0),
            Some(palette::SUCCESS_PEAK),
            None,
            None,
            Some(palette::BG_1),
            None,
            None,
            None,
            None,
        );
    }

    pub fn dialog_exit(&self, parent: Rect, buf: &mut Buffer) {
        if !self.exit_alert_visible {
            return;
        }

        Self::_popup_ex(
            parent,
            buf,
            None,
            "Quit the UI?",
            Some(palette::POPUP_BG_BASE),
            Alignment::Center,
            self.exit_alert_choice.clone(),
            AlertButtons::OkCancel,
            Some(36),
            Some(palette::PROCESSING_PEAK),
            Some(ratatui::widgets::BorderType::Rounded),
            Some(palette::FG),
            None,
            Some("Yep!"),
            Some("Nope"),
            None,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
    }

    pub fn dialog_cluster_confirm(&self, parent: Rect, buf: &mut Buffer) {
        if !self.cluster_confirm_visible {
            return;
        }
        let (plain_text, styled_text): (String, Option<Text<'_>>) = match self.pending_cluster_action {
            1 => ("\nShut down every online minion\nin the entire cluster?".to_string(), None),
            2 => ("\nForce every online minion to drop\nand re-establish its connection?".to_string(), None),
            3 => {
                let host = self.selected_popup_minion().map(|r| Self::online_host(&r)).unwrap_or_else(|| "unknown".to_string());
                let plain = format!("\nDo you want to unregister {host} from this cluster?");
                let styled = Text::from(vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Do you want to unregister "),
                        Span::styled(host.clone(), Style::default().fg(palette::SUCCESS)),
                        Span::raw(" from this cluster?"),
                    ]),
                ]);
                (plain, Some(styled))
            }
            _ => return,
        };
        Self::_popup_ex(
            parent,
            buf,
            Some("Cluster Operation"),
            &plain_text,
            None,
            Alignment::Center,
            self.cluster_confirm_choice.clone(),
            AlertButtons::YesNo,
            Some(0),
            Some(palette::PROCESSING_PEAK),
            None,
            None,
            Some(palette::WHITE),
            None,
            None,
            styled_text,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
    }

    /// Draws a button in MS-DOS style (no shadow)
    pub(crate) fn format_button(label: &str) -> String {
        let trimmed: String = if label.chars().count() > 10 { label.chars().take(10).collect() } else { label.to_string() };
        let total_pad = 10 - trimmed.chars().count();
        let left_pad = total_pad / 2;
        format!("[{}{}{}]", " ".repeat(left_pad), trimmed, " ".repeat(total_pad - left_pad))
    }

    /// Draws quit popup area
    fn quit_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, text_align: Alignment, background: Option<Color>, choice: AlertResult,
        width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, text_align, choice, AlertButtons::Quit, width);
    }

    /// Draws ok/cancel popup area
    fn okcancel_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, choice: AlertResult, width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, Alignment::Center, choice, AlertButtons::OkCancel, width);
    }

    /// Draws yes/no popup area
    fn yesno_popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, choice: AlertResult, width: Option<u16>,
    ) {
        Self::_popup(parent, buf, title, text, background, Alignment::Center, choice, AlertButtons::YesNo, width);
    }

    /// Draws a popup area with custom border/text colours
    #[allow(clippy::too_many_arguments)]
    fn _popup_ex(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, text_align: Alignment, choice: AlertResult,
        buttons: AlertButtons, width: Option<u16>, border_color: Option<Color>, border_type: Option<ratatui::widgets::BorderType>,
        text_color: Option<Color>, title_color: Option<Color>, left_label: Option<&str>, right_label: Option<&str>, styled_text: Option<Text<'_>>,
        gradient: Option<(f32, &[Color])>,
    ) {
        let background = background.unwrap_or(palette::POPUP_BG_BASE);
        let border_color = border_color.unwrap_or(palette::BORDER);
        let border_type = border_type.unwrap_or(ratatui::widgets::BorderType::Rounded);
        let text_color = text_color.unwrap_or(palette::FG);
        let title_color = title_color.unwrap_or(palette::BLACK);
        let has_gradient = gradient.is_some();

        let text = format!("\n{text}");
        let text_lines = Self::get_text_lines(&text);
        let height = text_lines + 3;

        #[allow(clippy::unnecessary_unwrap)]
        let mut width = if width.is_none() { (parent.width / 4).max(20) } else { width.unwrap() };
        if width == 0 {
            width = Self::get_max_width_lines(&text) + 6;
        }

        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .title(if let Some(t) = title {
                Line::from(vec![
                    Span::styled("\u{E0B2}", Style::default().fg(border_color)),
                    Span::styled(t.to_string(), Style::default().fg(title_color).bg(border_color)),
                    Span::styled("\u{E0B0}", Style::default().fg(border_color)),
                ])
            } else {
                Line::from("")
            })
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(2))
            .style(if has_gradient { Style::default() } else { Style::default().bg(background) });

        let popup_inner = popup_block.inner(canvas);

        if let Some((angle, stops)) = gradient {
            let colors = blend_2d(canvas.width as usize, canvas.height as usize, angle, stops);
            for row in 0..canvas.height {
                for col in 0..canvas.width {
                    let idx = row as usize * canvas.width as usize + col as usize;
                    if let Some(cell) = buf.cell_mut(Position::new(canvas.x + col, canvas.y + row)) {
                        cell.set_bg(colors[idx]);
                    }
                }
            }
        }

        popup_block.render(canvas, buf);

        let text_bg = if has_gradient { Style::default().fg(text_color) } else { Style::default().fg(text_color).bg(background) };
        let vertical_chunks =
            Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(text_lines), Constraint::Length(1)]).split(popup_inner);

        let text_area = vertical_chunks[0];
        let button_area = vertical_chunks[1];
        if let Some(st) = styled_text {
            Paragraph::new(st).alignment(text_align).style(text_bg).render(text_area, buf);
        } else {
            Paragraph::new(text).alignment(text_align).style(text_bg).render(text_area, buf);
        }
        let (lbtn_label, rbtn_label) = match buttons {
            AlertButtons::YesNo => (Self::format_button(YES_LABEL), Self::format_button(NO_LABEL)),
            AlertButtons::OkCancel => (Self::format_button(left_label.unwrap_or(OK_LABEL)), Self::format_button(right_label.unwrap_or(CANCEL_LABEL))),
            AlertButtons::Ok => (Self::format_button(OK_LABEL), "".to_string()),
            AlertButtons::Quit => (Self::format_button(CLOSE_LABEL), "".to_string()),
            AlertButtons::Close => (Self::format_button(CLOSE_LABEL), "".to_string()),
        };

        let btn_w = button_area.width;

        let b_selected = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let b_unselected = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        if rbtn_label.is_empty() {
            Paragraph::new(lbtn_label.clone()).style(b_selected).render(
                Rect {
                    x: button_area.x + (btn_w.saturating_sub(lbtn_label.len() as u16)) / 2,
                    y: button_area.y,
                    width: lbtn_label.len() as u16,
                    height: 1,
                },
                buf,
            );
        } else {
            let button_splits = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length((btn_w.saturating_sub(lbtn_label.len() as u16 + 3 + rbtn_label.len() as u16)) / 2),
                    Constraint::Length(lbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                    Constraint::Length(3),
                    Constraint::Length(rbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                ])
                .split(button_area);

            let (left_style, right_style) = if choice == AlertResult::Default { (b_unselected, b_selected) } else { (b_selected, b_unselected) };

            Paragraph::new(lbtn_label).style(left_style).render(button_splits[1], buf);
            Paragraph::new(rbtn_label).style(right_style).render(button_splits[3], buf);
        }

        // MS-DOS style shadows
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        for idx in 0..width {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(height);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }

        for offset in 0..2 {
            for idx in 0..height {
                let sx = x.saturating_add(width).saturating_add(offset);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > max_x || sy > max_y {
                    continue;
                }
                if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                    cell.set_bg(palette::SHADOW_BG);
                    cell.set_fg(palette::SHADOW_FG);
                }
            }
        }
    }

    /// Draws a popup area
    fn _popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, text_align: Alignment, choice: AlertResult,
        buttons: AlertButtons, width: Option<u16>,
    ) {
        let background = background.unwrap_or(palette::POPUP_BG_BASE);

        let text = format!("\n{text}");
        let text_lines = Self::get_text_lines(&text);
        let height = text_lines + 3;

        #[allow(clippy::unnecessary_unwrap)]
        let mut width = if width.is_none() { (parent.width / 4).max(20) } else { width.unwrap() };
        if width == 0 {
            width = Self::get_max_width_lines(&text) + 6;
        }

        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .title(if let Some(t) = title {
                Line::from(vec![
                    Span::styled("\u{E0B2}", Style::default().fg(palette::BORDER)),
                    Span::styled(t.to_string(), Style::default().fg(palette::BLACK).bg(palette::BORDER)),
                    Span::styled("\u{E0B0}", Style::default().fg(palette::BORDER)),
                ])
            } else {
                Line::from("")
            })
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER))
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(background));

        let popup_inner = popup_block.inner(canvas);
        popup_block.render(canvas, buf);

        let vertical_chunks =
            Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(text_lines), Constraint::Length(1)]).split(popup_inner);

        let text_area = vertical_chunks[0];
        let button_area = vertical_chunks[1];

        Paragraph::new(text).alignment(text_align).style(Style::default().fg(palette::FG).bg(background)).render(text_area, buf);
        let (lbtn_label, rbtn_label) = match buttons {
            AlertButtons::YesNo => (Self::format_button(YES_LABEL), Self::format_button(NO_LABEL)),
            AlertButtons::OkCancel => (Self::format_button(OK_LABEL), Self::format_button(CANCEL_LABEL)),
            AlertButtons::Ok => (Self::format_button(OK_LABEL), "".to_string()),
            AlertButtons::Quit => (Self::format_button(CLOSE_LABEL), "".to_string()),
            AlertButtons::Close => (Self::format_button(CLOSE_LABEL), "".to_string()),
        };

        let btn_w = button_area.width;

        let b_selected = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let b_unselected = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        if rbtn_label.is_empty() {
            Paragraph::new(lbtn_label.clone()).style(b_selected).render(
                Rect {
                    x: button_area.x + (btn_w.saturating_sub(lbtn_label.len() as u16)) / 2,
                    y: button_area.y,
                    width: lbtn_label.len() as u16,
                    height: 1,
                },
                buf,
            );
        } else {
            let button_splits = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length((btn_w.saturating_sub(lbtn_label.len() as u16 + 3 + rbtn_label.len() as u16)) / 2),
                    Constraint::Length(lbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                    Constraint::Length(3),
                    Constraint::Length(rbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                ])
                .split(button_area);

            let (left_style, right_style) = if choice == AlertResult::Default { (b_unselected, b_selected) } else { (b_selected, b_unselected) };

            Paragraph::new(lbtn_label).style(left_style).render(button_splits[1], buf);
            Paragraph::new(rbtn_label).style(right_style).render(button_splits[3], buf);
        }

        // MS-DOS style shadows
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        for idx in 0..width {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(height);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }

        for offset in 0..2 {
            for idx in 0..height {
                let sx = x.saturating_add(width).saturating_add(offset);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > max_x || sy > max_y {
                    continue;
                }
                if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                    cell.set_bg(palette::SHADOW_BG);
                    cell.set_fg(palette::SHADOW_FG);
                }
            }
        }
    }
}

/// Wrap text to a maximum width, preserving leading whitespace per paragraph.
pub(crate) fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() || max_width < 4 {
        return vec![];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }
        let lead = &paragraph[..paragraph.len() - paragraph.trim_start().len()];
        let mut current = lead.to_string();
        for word in trimmed.split_whitespace() {
            if current.len() + 1 + word.len() > max_width {
                lines.push(std::mem::take(&mut current));
                current = lead.to_string();
            }
            if !current.is_empty() && current != lead {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}
