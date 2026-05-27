use super::{SysInspectUX, elements::AlertResult};
use indexmap::IndexMap;
use libsysinspect::console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget},
};
use ratatui_cheese::tree::{Tree, TreeGroup, TreeItem, TreeState};
use serde_json::Value;

#[derive(Default)]
enum AlertButtons {
    YesNo,
    OkCancel,
    #[default]
    Ok,
    Quit,
}

static YES_LABEL: &str = "Yes";
static NO_LABEL: &str = "No";
static OK_LABEL: &str = "OK";
static CANCEL_LABEL: &str = "Cancel";
static QUIT_LABEL: &str = "Quit";
static CLOSE_LABEL: &str = "Close";
static ONLINE_LABEL: &str = "Online";
static OFFLINE_LABEL: &str = "Offline";
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
            &format!("An unexpected error occurred:\n{}\n\nPlease check the logs for more information.", self.error_alert_message),
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

    /// Shorten a display string by preserving leading/trailing `edge` chars and replacing the middle with `...`.
    fn shorten_mid(value: &str, edge: usize) -> String {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= (edge * 2) + 3 {
            return value.to_string();
        }
        format!("{}...{}", chars.iter().take(edge).collect::<String>(), chars[chars.len().saturating_sub(edge)..].iter().collect::<String>())
    }

    /// Choose the preferred host label for an online-minion row.
    fn online_host(row: &ConsoleOnlineMinionRow) -> String {
        if !row.fqdn.trim().is_empty() {
            return row.fqdn.clone();
        }
        if !row.hostname.trim().is_empty() {
            return row.hostname.clone();
        }
        "unknown".to_string()
    }

    pub fn dialog_online_minions(&self, parent: Rect, buf: &mut Buffer) {
        if !self.online_minions_visible {
            return;
        }

        let rows = &self.online_minions_rows;
        let width = (parent.width * 3 / 4).max(60);
        let height = (parent.height * 3 / 4).max(15);
        let bg = Color::Green;

        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let filtered: Vec<&ConsoleOnlineMinionRow> = rows.iter().filter(|r| r.alive == self.online_minions_show_alive).collect();
        let title = if self.online_minions_show_alive { "Online Minions" } else { "Offline Minions" };
        let block = Block::default()
            .title(format!(" {} ({}) ", title, filtered.len()))
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(Color::Black).bg(Color::Gray))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray))
            .style(Style::default().bg(bg));

        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let close_label = Self::format_button(CLOSE_LABEL);
        let toggle_label = Self::format_button(if self.online_minions_show_alive { OFFLINE_LABEL } else { ONLINE_LABEL });
        let btn_gap = 3u16;

        let vert = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        let main_area = vert[0];
        let btn_area = vert[1];

        let horiz =
            Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(55), Constraint::Percentage(45)]).split(main_area);

        let table_area = horiz[0];
        let info_area = horiz[1];

        let host_w = filtered.iter().map(|r| Self::online_host(r).chars().count() as u16).max().unwrap_or(4).max(4);
        let ip_w = filtered.iter().map(|r| if r.ip.is_empty() { 7 } else { r.ip.chars().count() as u16 }).max().unwrap_or(2).max(2);
        let ver_w = filtered
            .iter()
            .map(|r| {
                if r.outdated && !r.version.is_empty() && !r.target_version.is_empty() {
                    format!("{} -> {}", r.version, r.target_version).chars().count() as u16
                } else if r.version.is_empty() {
                    1
                } else {
                    r.version.chars().count() as u16
                }
            })
            .max()
            .unwrap_or(7)
            .max(7);
        let id_w = filtered.iter().map(|r| Self::shorten_mid(&r.minion_id, 4).chars().count() as u16).max().unwrap_or(2).max(2);

        let col_spacing = 2u16;
        let cols = [
            Constraint::Length(host_w + col_spacing),
            Constraint::Length(ip_w + col_spacing),
            Constraint::Length(ver_w + col_spacing),
            Constraint::Length(id_w),
        ];

        let sel_style = if self.online_minions_focus == 2 {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(Color::Gray)
        };
        let norm_style = Style::default().fg(Color::White).bg(bg);

        let table_rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                let host = Self::online_host(r);
                let ip = if r.ip.is_empty() { "unknown".to_string() } else { r.ip.clone() };
                let version = if r.outdated && !r.version.is_empty() && !r.target_version.is_empty() {
                    format!("{} -> {}", r.version, r.target_version)
                } else if r.version.is_empty() {
                    "-".to_string()
                } else {
                    r.version.clone()
                };
                let id = Self::shorten_mid(&r.minion_id, 4);

                let sty = if idx == self.online_minions_selected { sel_style } else { norm_style };
                Row::new(vec![host, ip, version, id]).style(sty)
            })
            .collect();

        let vis_rows = table_area.height as usize;
        let start = if self.online_minions_selected < vis_rows { 0 } else { (self.online_minions_selected + 1).saturating_sub(vis_rows) };
        let end = (start + vis_rows).min(table_rows.len());
        let displayed: Vec<Row> = if start < table_rows.len() { table_rows[start..end].to_vec() } else { vec![] };

        Widget::render(Table::new(displayed, cols).column_spacing(2).style(Style::default().bg(bg)), table_area, buf);

        let scroller_area = Rect { x: table_area.right().saturating_sub(1), y: table_area.y, width: 1, height: table_area.height };
        let mut sr_scroller = ScrollbarState::default().content_length(table_rows.len()).position(self.online_minions_selected);
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            scroller_area,
            buf,
            &mut sr_scroller,
        );

        Self::_render_minion_info_panel(self, info_area, buf);

        let close_w = close_label.len() as u16;
        let toggle_w = toggle_label.len() as u16;
        let total_btn_width = close_w + btn_gap + toggle_w;
        let btn_start_x = btn_area.x + (btn_area.width.saturating_sub(total_btn_width)) / 2;
        let active_style = Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
        let passive_style = Style::default().fg(Color::White).bg(bg);

        let (close_style, toggle_style) = if self.online_minions_focus == 0 {
            (active_style, passive_style)
        } else if self.online_minions_focus == 1 {
            (passive_style, active_style)
        } else {
            (passive_style, passive_style)
        };

        Paragraph::new(close_label).style(close_style).render(Rect { x: btn_start_x, y: btn_area.y, width: close_w, height: 1 }, buf);
        Paragraph::new(toggle_label)
            .style(toggle_style)
            .render(Rect { x: btn_start_x + close_w + btn_gap, y: btn_area.y, width: toggle_w, height: 1 }, buf);

        for idx in 0..width {
            let cell = buf.cell_mut(Position::new(x + 2 + idx, y + height)).unwrap();
            cell.set_bg(Color::Black);
            cell.set_fg(Color::DarkGray);
        }
        for offset in 0..2 {
            for idx in 0..height {
                let cell = buf.cell_mut(Position::new(x + width + offset, y + idx + 1)).unwrap();
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
            }
        }
    }

    fn _render_minion_info_panel(&self, area: Rect, buf: &mut Buffer) {
        let info_bg = Color::Cyan;
        let is_active = self.online_minions_focus == 3;
        let border_fg = if is_active { Color::White } else { Color::LightCyan };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(border_fg))
            .style(Style::default().bg(info_bg));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.online_minions_info_rows.is_empty() {
            Paragraph::new("No minion selected").style(Style::default().fg(Color::Black).bg(info_bg)).render(inner, buf);
            return;
        }

        let groups = Self::build_info_tree(&self.online_minions_info_rows);
        let n_groups = groups.len();
        let tree = Tree::default().groups(groups);

        if let Some(ref mut ts) = self.online_minions_tree_state.clone() {
            let mut state = ts.clone();
            StatefulWidget::render(&tree, inner, buf, &mut state);
        } else {
            let mut state = TreeState::all_expanded(n_groups);
            StatefulWidget::render(&tree, inner, buf, &mut state);
        }
    }

    /// Build tree groups from minion info rows, grouping by the first dot-separated segment.
    pub fn build_info_tree(rows: &[ConsoleMinionInfoRow]) -> Vec<TreeGroup> {
        let mut sorted = rows.to_vec();
        sorted.sort_by(|a, b| a.key.cmp(&b.key));

        let mut groups: IndexMap<String, Vec<&ConsoleMinionInfoRow>> = IndexMap::new();
        for r in &sorted {
            let prefix = r.key.split('.').next().unwrap_or(&r.key).to_string();
            groups.entry(prefix).or_default().push(r);
        }

        groups
            .into_iter()
            .map(|(prefix, items)| {
                let children: Vec<TreeItem> = items
                    .iter()
                    .map(|r| {
                        let v = Self::_info_value_str(&r.value);
                        TreeItem::new(format!("{}: {}", r.key, v))
                    })
                    .collect();
                TreeGroup::new(TreeItem::new(prefix)).children(children)
            })
            .collect()
    }

    fn _info_value_str(value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(f) => if *f { "yes" } else { "no" }.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(items) => {
                if items.is_empty() {
                    return "[]".to_string();
                }
                items.iter().map(Self::_info_value_str).collect::<Vec<_>>().join(", ")
            }
            Value::Object(_) => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
        }
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

    /// Draws a popup area
    fn _popup(
        parent: Rect, buf: &mut Buffer, title: Option<&str>, text: &str, background: Option<Color>, text_align: Alignment, choice: AlertResult,
        buttons: AlertButtons, width: Option<u16>,
    ) {
        let background = background.unwrap_or(Color::Red);

        let text = format!("\n{text}");
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

        let vertical_chunks =
            Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(text_lines), Constraint::Length(1)]).split(popup_inner);

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
