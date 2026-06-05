use super::{SysInspectUX, palette};
use indexmap::IndexMap;
use libsysinspect::{
    console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow},
    traits::TraitSource,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget},
};
use ratatui_cheese::tree::{Tree, TreeGroup, TreeItem, TreeState, TreeStyles};
use serde_json::Value;

static CLOSE_LABEL: &str = "Close";
static ONLINE_LABEL: &str = "Online";
static OFFLINE_LABEL: &str = "Offline";

impl SysInspectUX {
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
        let title_style =
            if self.online_minions_focus == 3 { Style::default().fg(Color::Black).bg(bg) } else { Style::default().fg(Color::Black).bg(Color::Gray) };
        let block = Block::default()
            .title(format!(" {} ({}) ", title, filtered.len()))
            .title_alignment(Alignment::Center)
            .title_style(title_style)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray))
            .style(Style::default().bg(bg));

        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let close_label = Self::format_button(CLOSE_LABEL);
        let toggle_label = Self::format_button(if self.online_minions_show_alive { OFFLINE_LABEL } else { ONLINE_LABEL });
        let btn_gap = 3u16;

        let col_spacing = 2u16;
        let max_w = 15u16;

        let vert = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        let main_area = vert[0];
        let btn_area = vert[1];

        if filtered.is_empty() {
            let label = if self.online_minions_show_alive { "No minions online" } else { "No minions offline" };
            let v_pad = main_area.height.saturating_sub(1) / 2;
            let centered = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(v_pad), Constraint::Length(1), Constraint::Min(0)])
                .split(main_area);
            Paragraph::new(label).alignment(Alignment::Center).style(Style::default().fg(Color::White).bg(bg)).render(centered[1], buf);
        } else {
            let ip_data: Vec<String> = filtered.iter().map(|r| Self::_fmt_ip(&r.ip)).collect();
            let host_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&Self::online_host(r), max_w as usize)).collect();
            let ver_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&Self::_fmt_version(r), max_w as usize)).collect();
            let id_data: Vec<String> = filtered.iter().map(|r| Self::shorten_mid(&r.minion_id, 4)).collect();
            let os_data: Vec<String> = filtered
                .iter()
                .map(|r| {
                    let name = if r.os_name.is_empty() { "-" } else { r.os_name.as_str() };
                    let dist = if r.os_distribution.is_empty() { "-" } else { r.os_distribution.as_str() };
                    let s = format!("{}/{}", name, dist);
                    Self::_trunc_ellipsis(&s, max_w as usize)
                })
                .collect();
            let osv_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&r.os_version, max_w as usize)).collect();
            let ker_data: Vec<String> = filtered.iter().map(|r| r.kernel.clone()).collect();

            let ip_w = ip_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
            let host_w = host_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(4).max(4);
            let ver_w = ver_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(7).min(max_w);
            let id_w = id_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);

            if self.online_minions_info_visible {
                let base_w: Vec<u16> = vec![ip_w, host_w, ver_w, id_w];
                let table_w = base_w.iter().sum::<u16>() + col_spacing * 3 + 1;
                let horiz = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(table_w), Constraint::Min(0)])
                    .split(main_area);
                let table_area = horiz[0];
                let info_area = horiz[1];
                let cols = base_w.into_iter().map(Constraint::Length).collect::<Vec<_>>();

                Self::_render_table(self, &filtered, table_area, buf, bg, &cols, &ip_data, &host_data, &ver_data, &id_data);
                Self::_render_minion_info_panel(self, info_area, buf);
            } else {
                let os_w = os_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
                let osv_w = osv_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
                let ker_w = ker_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
                let base_w: Vec<u16> = vec![ip_w, host_w, ver_w, id_w, os_w, osv_w, ker_w];
                let mut cols: Vec<Constraint> = base_w.into_iter().map(Constraint::Length).collect();
                cols.push(Constraint::Min(1));
                let all_data: [&[String]; 7] = [&ip_data, &host_data, &ver_data, &id_data, &os_data, &osv_data, &ker_data];
                Self::_render_table_wide(self, &filtered, main_area, buf, bg, &cols, &all_data);
            }
        }

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
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
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
                    cell.set_bg(Color::Black);
                    cell.set_fg(Color::DarkGray);
                }
            }
        }
    }

    fn _render_minion_info_panel(&self, area: Rect, buf: &mut Buffer) {
        let info_bg = Color::Blue;
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
        let styles = TreeStyles {
            parent: Style::default().fg(Color::LightCyan).bg(info_bg).add_modifier(Modifier::BOLD),
            child: Style::default().fg(Color::White).bg(info_bg).add_modifier(Modifier::BOLD),
            selected: Style::default().fg(Color::Black).bg(Color::LightCyan),
            chevron: Style::default().fg(Color::LightCyan).bg(info_bg),
            chevron_active: Style::default().fg(Color::LightCyan).bg(info_bg),
            chevron_dim: Style::default().fg(Color::LightCyan).bg(info_bg),
            count: Style::default().fg(Color::White).bg(info_bg),
            icon: Style::default().fg(Color::LightYellow).bg(info_bg),
        };
        let tree = Tree::default().groups(groups).styles(styles).chevron_collapsed("▶ ").chevron_expanded("▼ ");

        if let Some(ref ts) = self.online_minions_tree_state {
            let mut state = ts.clone();
            StatefulWidget::render(&tree, inner, buf, &mut state);
        } else {
            let mut state = TreeState::all_expanded(n_groups);
            StatefulWidget::render(&tree, inner, buf, &mut state);
        }
    }

    /// Build tree groups from minion info rows.
    /// Preset traits are grouped by first dot-segment, Static keys under "Static", Function keys under "Scripts".
    pub fn build_info_tree(rows: &[ConsoleMinionInfoRow]) -> Vec<TreeGroup> {
        let mut sorted = rows.to_vec();
        sorted.sort_by(|a, b| a.key.cmp(&b.key));

        let mut groups: IndexMap<String, Vec<&ConsoleMinionInfoRow>> = IndexMap::new();
        for r in &sorted {
            let prefix = match r.source {
                TraitSource::Static => "Static".to_string(),
                TraitSource::Function => "Scripts".to_string(),
                TraitSource::Preset => {
                    let seg = r.key.split('.').next().unwrap_or(&r.key);
                    let mut chars = seg.chars();
                    match chars.next() {
                        None => seg.to_string(),
                        Some(c) => c.to_uppercase().chain(chars).collect(),
                    }
                }
            };
            groups.entry(prefix).or_default().push(r);
        }

        let mut ordered: Vec<(String, Vec<&ConsoleMinionInfoRow>)> = groups.into_iter().collect();
        ordered.sort_by(|(a, _), (b, _)| a.cmp(b));

        ordered
            .into_iter()
            .map(|(prefix, items)| {
                let max_key = items.iter().map(|r| r.key.len()).max().unwrap_or(0);
                let children: Vec<TreeItem> = items
                    .iter()
                    .map(|r| {
                        let v = Self::_info_value_str(&r.value);
                        let key = format!("{:<width$}: ", r.key, width = max_key);
                        TreeItem::new(v).icon(key)
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

    fn _fmt_ip(ip: &str) -> String {
        if ip.is_empty() {
            return "unknown".to_string();
        }
        if ip.len() > 15 {
            return ip.chars().take(15).collect::<String>() + "…";
        }
        ip.to_string()
    }

    fn _fmt_version(r: &ConsoleOnlineMinionRow) -> String {
        if r.outdated && !r.version.is_empty() && !r.target_version.is_empty() {
            format!("{} -> {}", r.version, r.target_version)
        } else if r.version.is_empty() {
            "-".to_string()
        } else {
            r.version.clone()
        }
    }

    fn _trunc_ellipsis(s: &str, max: usize) -> String {
        if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>()) }
    }

    #[allow(clippy::too_many_arguments)]
    fn _render_table(
        &self, filtered: &[&ConsoleOnlineMinionRow], area: Rect, buf: &mut Buffer, bg: Color, cols: &[Constraint], ip_data: &[String],
        host_data: &[String], ver_data: &[String], id_data: &[String],
    ) {
        let sel_style = if self.online_minions_focus == 2 {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(Color::Gray)
        };
        let norm_style = Style::default().fg(Color::White).bg(bg);
        let rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let sty = if idx == self.online_minions_selected { sel_style } else { norm_style };
                Row::new(vec![ip_data[idx].as_str(), host_data[idx].as_str(), ver_data[idx].as_str(), id_data[idx].as_str()]).style(sty)
            })
            .collect();
        Self::_render_scrollable_table(area, buf, bg, cols, &rows, self.online_minions_selected);
    }

    fn _render_table_wide(
        &self, filtered: &[&ConsoleOnlineMinionRow], area: Rect, buf: &mut Buffer, bg: Color, cols: &[Constraint], data: &[&[String]; 7],
    ) {
        let sel_style = if self.online_minions_focus == 2 {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(Color::Gray)
        };
        let norm_style = Style::default().fg(Color::White).bg(bg);
        let rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let sty = if idx == self.online_minions_selected { sel_style } else { norm_style };
                Row::new(vec![
                    data[0][idx].as_str(),
                    data[1][idx].as_str(),
                    data[2][idx].as_str(),
                    data[3][idx].as_str(),
                    data[4][idx].as_str(),
                    data[5][idx].as_str(),
                    data[6][idx].as_str(),
                    "",
                ])
                .style(sty)
            })
            .collect();
        Self::_render_scrollable_table(area, buf, bg, cols, &rows, self.online_minions_selected);
    }

    fn _render_scrollable_table(area: Rect, buf: &mut Buffer, bg: Color, cols: &[Constraint], rows: &[Row], selected: usize) {
        let vis_rows = area.height as usize;
        let start = if selected < vis_rows { 0 } else { (selected + 1).saturating_sub(vis_rows) };
        let end = (start + vis_rows).min(rows.len());
        let displayed: Vec<Row> = if start < rows.len() { rows[start..end].to_vec() } else { vec![] };
        Widget::render(Table::new(displayed, cols).column_spacing(2).style(Style::default().bg(bg)), area, buf);
        let scroller_area = Rect { x: area.right().saturating_sub(1), y: area.y, width: 1, height: area.height };
        let mut scroller = ScrollbarState::default().content_length(rows.len()).position(selected);
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_3))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(scroller_area, buf, &mut scroller);
    }
}
