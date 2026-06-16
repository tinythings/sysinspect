use super::{SysInspectUX, elements::AlertResult, palette};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_glamour::color::blend_2d;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub(crate) enum DialogFormWidget {
    Checkbox { label: String, checked: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogFormFocus {
    Widget(usize),
    LeftButton,
    RightButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogFormAlignment {
    Left,
    Center,
}

impl DialogFormFocus {
    pub(crate) fn next(self, widgets_len: usize, has_right_button: bool) -> Self {
        let total = widgets_len + 1 + usize::from(has_right_button);
        Self::from_index((self.index(widgets_len, has_right_button) + 1) % total, widgets_len, has_right_button)
    }

    pub(crate) fn prev(self, widgets_len: usize, has_right_button: bool) -> Self {
        let total = widgets_len + 1 + usize::from(has_right_button);
        Self::from_index((self.index(widgets_len, has_right_button) + total - 1) % total, widgets_len, has_right_button)
    }

    fn index(self, widgets_len: usize, has_right_button: bool) -> usize {
        match self {
            Self::Widget(idx) => idx.min(widgets_len.saturating_sub(1)),
            Self::LeftButton => widgets_len,
            Self::RightButton => widgets_len + usize::from(has_right_button),
        }
    }

    fn from_index(index: usize, widgets_len: usize, has_right_button: bool) -> Self {
        if index < widgets_len {
            Self::Widget(index)
        } else if has_right_button && index == widgets_len + 1 {
            Self::RightButton
        } else {
            Self::LeftButton
        }
    }
}

#[derive(Default)]
enum AlertButtons {
    YesNo,
    OkCancel,
    #[default]
    Ok,
    Quit,
    Close,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct PopupButtonRects {
    pub left_button: Option<Rect>,
    pub right_button: Rect,
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
        let rects = Self::_popup_ex(
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
            Some((15.0, &[palette::BG_2, palette::BG_1] as &[Color])),
        );
        self.popup_button_rects.set(Some(rects));
    }

    pub fn dialog_info(&self, parent: Rect, buf: &mut Buffer, title: &str, text: &str, styled_text: Option<Text<'static>>, quit_button: bool) {
        let rects = Self::_popup_ex(
            parent,
            buf,
            Some(title),
            text,
            None,
            Alignment::Left,
            AlertResult::Quit,
            if quit_button { AlertButtons::Quit } else { AlertButtons::Close },
            Some(0),
            Some(palette::SUCCESS),
            None,
            None,
            Some(palette::BG_1),
            None,
            None,
            styled_text,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
        self.popup_button_rects.set(Some(rects));
    }

    pub fn dialog_purge(&self, parent: Rect, buf: &mut Buffer) {
        if !self.purge_alert_visible {
            return;
        }
        let rects = Self::_popup_ex(
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
        self.popup_button_rects.set(Some(rects));
    }

    pub fn dialog_help(&self, parent: Rect, buf: &mut Buffer) {
        if !self.help_popup_visible {
            return;
        }
        let lines: Vec<Line<'static>> = vec![
            Self::help_line("c", "Open the query composer: pick a model, target, and"),
            Self::help_line("", "state to run across your machines."),
            Line::from(""),
            Self::help_line("h", "Show this help window."),
            Line::from(""),
            Self::help_line("m", "Open the master menu: logs, registration, artefacts,"),
            Self::help_line("", "cluster upgrades."),
            Line::from(""),
            Self::help_line("o", "Open the minions list with filter, tagging, and"),
            Self::help_line("", "detailed per-machine inspection."),
            Line::from(""),
            Self::help_line("p", "Purge all locally stored records to free up space."),
            Line::from(""),
            Self::help_line("q", "Exit the Sysinspect TUI."),
            Line::from(""),
            Self::help_line("Esc", "Close popups or go back.  Press twice to quit."),
            Line::from(""),
            Self::help_line("Enter", "Open the selected item to drill into details:"),
            Self::help_line("", "cycles show machines, machines show events,"),
            Self::help_line("", "events show full data."),
            Line::from(""),
            Self::help_line("Up/Down", "Navigate through list items in the active panel."),
            Line::from(""),
            Self::help_line("Left/Right", "Switch between panels: Calls, Machines, Results,"),
            Self::help_line("", "Data."),
            Line::from(""),
            Self::help_line("Tab", "From Action Results, view full event data."),
            Line::from(""),
            Self::help_line("Ctrl+O", "Open master logs directly from the master."),
            Line::from(""),
            Self::help_line("Ctrl+L", "Open locally saved master logs for offline viewing."),
            Line::from(""),
            Self::help_line("Ctrl+R", "Open the registration form to add a new machine."),
            Line::from(""),
            Self::help_line("Ctrl+A", "Open the Artefacts Manager: modules, libraries,"),
            Self::help_line("", "models, profiles, platform builds."),
            Line::from(""),
            Self::help_line("Ctrl+U", "Run a cluster upgrade across your machines."),
            Line::from(""),
            Line::from(vec![Span::styled("For bug reporting and project updates, visit:", Style::default().fg(palette::GRAY_1))]),
            Line::from(vec![Span::styled("https://github.com/tinythings/sysinspect", Style::default().fg(palette::GRAY_1))]),
        ];

        let total = lines.len();
        let max_text_h = parent.height.saturating_sub(8);
        let max_scroll = total.saturating_sub(max_text_h as usize);
        let scroll = self.help_popup_scroll.get().min(max_scroll);
        self.help_popup_scroll.set(scroll);
        let visible: Vec<Line> = lines.into_iter().skip(scroll).take(max_text_h as usize).collect();
        let visible_h = visible.len() as u16;

        let w = (parent.width * 75 / 100).max(60).min(parent.width.saturating_sub(2));
        let h = visible_h.saturating_add(3);
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(palette::SUCCESS_PEAK))
            .title(Line::from(vec![
                Span::styled("\u{E0B2}", Style::default().fg(palette::SUCCESS_PEAK)),
                Span::styled("Help", Style::default().fg(palette::BLACK).bg(palette::SUCCESS_PEAK).add_modifier(Modifier::BOLD)),
                Span::styled("\u{E0B0}", Style::default().fg(palette::SUCCESS_PEAK)),
            ]))
            .style(Style::default().bg(palette::BG_1));
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let text_inner = Rect::new(inner.x + 2, inner.y, inner.width.saturating_sub(2), inner.height);
        Paragraph::new(Text::from(visible)).alignment(Alignment::Left).render(text_inner, buf);

        if total > max_text_h as usize {
            let sb_x = inner.right().saturating_sub(1);
            let mut sb_state = ScrollbarState::new(total).position(scroll);
            StatefulWidget::render(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_symbol(Some("\u{28FF}"))
                    .thumb_symbol("█")
                    .track_style(Style::default().bg(palette::BG_2))
                    .thumb_style(Style::default().fg(palette::GRAY_1)),
                Rect::new(sb_x, inner.y, 1, inner.height),
                buf,
                &mut sb_state,
            );
        }

        // MS-DOS shadow
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);
        for idx in 0..w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(h);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }
        for offset in 0..2u16 {
            for idx in 0..h {
                let sx = x.saturating_add(w).saturating_add(offset);
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

    fn help_line(key: &str, desc: &str) -> Line<'static> {
        let key_w = 11usize;
        let key_padded = format!("{:width$}", key, width = key_w);
        Line::from(vec![
            Span::styled(key_padded, Style::default().fg(palette::WARNING_PEAK).add_modifier(Modifier::BOLD)),
            Span::raw(""),
            Span::styled(desc.to_string(), Style::default().fg(palette::GRAY_2)),
        ])
    }

    pub fn dialog_exit(&self, parent: Rect, buf: &mut Buffer) {
        if !self.exit_alert_visible {
            return;
        }

        let rects = Self::_popup_ex(
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
        self.popup_button_rects.set(Some(rects));
    }

    pub fn dialog_cluster_confirm(&self, parent: Rect, buf: &mut Buffer) {
        if !self.cluster_confirm_visible {
            return;
        }
        let rects = match self.pending_cluster_action {
            1 => Self::_popup_ex(
                parent,
                buf,
                Some("Cluster Operation"),
                "\nShut down every online minion\nin the entire cluster?",
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
                None,
                Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
            ),
            2 => Self::_popup_ex(
                parent,
                buf,
                Some("Cluster Operation"),
                "\nForce every online minion to drop\nand re-establish its connection?",
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
                None,
                Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
            ),
            3 => {
                let host = self.selected_popup_minion().map(|r| Self::online_host(&r)).unwrap_or_else(|| "unknown".to_string());
                let styled = Text::from(vec![Line::from(vec![
                    Span::raw("Do you want to unregister "),
                    Span::styled(host, Style::default().fg(palette::SUCCESS)),
                    Span::raw(" from this cluster?"),
                ])]);
                Self::_popup_widgets(
                    parent,
                    buf,
                    Some("Cluster Operation"),
                    styled,
                    &[DialogFormWidget::Checkbox { label: "Remove client from the host".to_string(), checked: self.delete_force_remove }],
                    Some(self.cluster_confirm_form_focus),
                    Alignment::Center,
                    DialogFormAlignment::Center,
                    Some(palette::PROCESSING_PEAK),
                    Some(palette::WHITE),
                    Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
                )
            }
            _ => return,
        };
        self.popup_button_rects.set(Some(rects));
    }

    pub fn dialog_delete_progress(&self, parent: Rect, buf: &mut Buffer) {
        if !self.delete_progress.visible {
            return;
        }

        let text = Line::from(vec![Span::styled(
            format!("{} {}", self.delete_progress.spinner.view(), self.delete_progress.message),
            Style::default().fg(palette::FG),
        )]);
        let width = (UnicodeWidthStr::width(self.delete_progress.message.as_str()) as u16 + 12).max(38);
        let height = 5u16;
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(palette::PROCESSING_PEAK))
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(palette::POPUP_BG_BASE));
        let popup_inner = popup_block.inner(canvas);
        popup_block.render(canvas, buf);

        let [_, text_area, _]: [Rect; 3] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
            .split(popup_inner)
            .as_ref()
            .try_into()
            .unwrap();
        Paragraph::new(text).alignment(Alignment::Center).render(text_area, buf);

        Self::draw_popup_shadow(buf, canvas, height);
    }

    pub fn dialog_cluster_upgrade_progress(&self, parent: Rect, buf: &mut Buffer) {
        if !self.cluster_upgrade_progress.visible {
            return;
        }

        let text = Line::from(vec![Span::styled(
            format!("{} {}", self.cluster_upgrade_progress.spinner.view(), self.cluster_upgrade_progress.message),
            Style::default().fg(palette::FG),
        )]);
        let width = (UnicodeWidthStr::width(self.cluster_upgrade_progress.message.as_str()) as u16 + 12).max(48);
        let height = 5u16;
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(palette::WARNING_PEAK))
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(palette::POPUP_BG_BASE));
        let popup_inner = popup_block.inner(canvas);
        popup_block.render(canvas, buf);

        let [_, text_area, _]: [Rect; 3] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
            .split(popup_inner)
            .as_ref()
            .try_into()
            .unwrap();
        Paragraph::new(text).alignment(Alignment::Center).render(text_area, buf);

        Self::draw_popup_shadow(buf, canvas, height);
    }

    pub fn dialog_master_confirm(&self, parent: Rect, buf: &mut Buffer) {
        if !self.master_confirm_visible {
            return;
        }
        let text = match self.master_confirm_action {
            1 => "Start the master in daemon mode?",
            2 => "Restart the master?\n\nThis will stop the daemon and start it again.",
            3 => "Stop the master?\n\nThis will terminate the daemon process.",
            _ => return,
        };
        let rects = Self::_popup_ex(
            parent,
            buf,
            Some("Master Operation"),
            text,
            None,
            Alignment::Center,
            self.master_confirm_choice.clone(),
            AlertButtons::YesNo,
            Some(50),
            Some(palette::PROCESSING_PEAK),
            None,
            None,
            Some(palette::FG),
            None,
            None,
            None,
            Some((10.0, &[palette::GRAY_0, palette::BG_2] as &[Color])),
        );
        self.popup_button_rects.set(Some(rects));
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
    ) -> PopupButtonRects {
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
            AlertButtons::Quit => (Self::format_button(QUIT_LABEL), "".to_string()),
            AlertButtons::Close => (Self::format_button(CLOSE_LABEL), "".to_string()),
        };

        let btn_w = button_area.width;

        let b_selected = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let b_unselected = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let popup_rects = if rbtn_label.is_empty() {
            let rect = Rect {
                x: button_area.x + (btn_w.saturating_sub(lbtn_label.len() as u16)) / 2,
                y: button_area.y,
                width: lbtn_label.len() as u16,
                height: 1,
            };
            Paragraph::new(lbtn_label.clone()).style(b_selected).render(rect, buf);
            PopupButtonRects { left_button: None, right_button: rect }
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
            PopupButtonRects { left_button: Some(button_splits[1]), right_button: button_splits[3] }
        };

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

        popup_rects
    }

    fn _popup_widgets(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: Text<'_>, widgets: &[DialogFormWidget], focus: Option<DialogFormFocus>,
        text_align: Alignment, widget_align: DialogFormAlignment, border_color: Option<Color>, title_color: Option<Color>,
        gradient: Option<(f32, &[Color])>,
    ) -> PopupButtonRects {
        let background = palette::POPUP_BG_BASE;
        let border_color = border_color.unwrap_or(palette::BORDER);
        let title_color = title_color.unwrap_or(palette::BLACK);
        let has_gradient = gradient.is_some();
        let text_lines = text.lines.len() as u16;
        let widget_rows = widgets.len() as u16;
        let height = text_lines + widget_rows + 6;
        let mut width = Self::get_max_width_text(&text).max(Self::get_max_width_widgets(widgets)) + 6;
        width = width.max((parent.width / 4).max(20));

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
            .border_type(ratatui::widgets::BorderType::Rounded)
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(text_lines),
                Constraint::Length(1),
                Constraint::Length(widget_rows),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(popup_inner);

        Paragraph::new(text).alignment(text_align).style(Style::default().fg(palette::FG)).render(chunks[1], buf);
        Self::render_form_widgets(buf, chunks[3], background, widgets, focus, widget_align);

        let lbtn_label = Self::format_button(YES_LABEL);
        let rbtn_label = Self::format_button(NO_LABEL);
        let button_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((chunks[5].width.saturating_sub(lbtn_label.len() as u16 + 3 + rbtn_label.len() as u16)) / 2),
                Constraint::Length(lbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
                Constraint::Length(3),
                Constraint::Length(rbtn_label.len().try_into().unwrap_or(DEFAULT_BUTTON_WIDTH)),
            ])
            .split(chunks[5]);
        let b_selected = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let b_unselected = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);
        let (left_style, right_style) = match focus.unwrap_or(DialogFormFocus::LeftButton) {
            DialogFormFocus::LeftButton => (b_selected, b_unselected),
            DialogFormFocus::RightButton => (b_unselected, b_selected),
            DialogFormFocus::Widget(_) => (b_unselected, b_unselected),
        };
        Paragraph::new(lbtn_label).style(left_style).render(button_splits[1], buf);
        Paragraph::new(rbtn_label).style(right_style).render(button_splits[3], buf);

        Self::draw_popup_shadow(buf, canvas, height);
        PopupButtonRects { left_button: Some(button_splits[1]), right_button: button_splits[3] }
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
            AlertButtons::Quit => (Self::format_button(QUIT_LABEL), "".to_string()),
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

    fn render_form_widgets(
        buf: &mut Buffer, area: Rect, _background: Color, widgets: &[DialogFormWidget], focus: Option<DialogFormFocus>,
        alignment: DialogFormAlignment,
    ) {
        for (idx, widget) in widgets.iter().enumerate() {
            let y = area.y + idx as u16;
            let is_focused = matches!(focus, Some(DialogFormFocus::Widget(focused)) if focused == idx);

            match widget {
                DialogFormWidget::Checkbox { label, checked } => {
                    let checkbox = if *checked { "▣" } else { "□" };
                    let row_text = format!("{checkbox}  {label}");
                    let row_width = row_text.chars().count() as u16;
                    let start_x = match alignment {
                        DialogFormAlignment::Left => area.x,
                        DialogFormAlignment::Center => area.x + area.width.saturating_sub(row_width) / 2,
                    };
                    let row_style = if is_focused {
                        Style::default().fg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::FG)
                    };
                    let checkbox_style = if is_focused {
                        row_style
                    } else if *checked {
                        Style::default().fg(palette::SUCCESS)
                    } else {
                        Style::default().fg(palette::GRAY_1)
                    };
                    buf.set_string(start_x, y, checkbox, checkbox_style);
                    buf.set_string(start_x + 3, y, label, row_style);
                }
            }
        }
    }

    fn get_max_width_widgets(widgets: &[DialogFormWidget]) -> u16 {
        widgets
            .iter()
            .map(|widget| match widget {
                DialogFormWidget::Checkbox { label, .. } => 3 + label.chars().count() as u16,
            })
            .max()
            .unwrap_or(0)
    }

    fn get_max_width_text(text: &Text<'_>) -> u16 {
        text.lines.iter().map(|l| UnicodeWidthStr::width(l.to_string().as_str()) as u16).max().unwrap_or(0)
    }

    fn draw_popup_shadow(buf: &mut Buffer, canvas: Rect, height: u16) {
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);
        for idx in 0..canvas.width {
            let sx = canvas.x.saturating_add(2).saturating_add(idx);
            let sy = canvas.y.saturating_add(height);
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
                let sx = canvas.x.saturating_add(canvas.width).saturating_add(offset);
                let sy = canvas.y.saturating_add(idx).saturating_add(1);
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
