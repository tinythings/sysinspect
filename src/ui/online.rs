use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use indexmap::IndexMap;
use libsysinspect::{
    console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow},
    traits::TraitSource,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget},
};
use ratatui_cheese::{
    input::{Input, InputState},
    tree::{TreeGroup, TreeItem},
};
use serde_json::Value;

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
    pub(crate) fn online_host(row: &ConsoleOnlineMinionRow) -> String {
        if !row.fqdn.trim().is_empty() {
            return row.fqdn.clone();
        }
        if !row.hostname.trim().is_empty() {
            return row.hostname.clone();
        }
        "unknown".to_string()
    }

    pub fn dialog_minions(&self, parent: Rect, buf: &mut Buffer) {
        if !self.minions_visible {
            return;
        }

        let filter_str = self.minions_filter_input.value();
        let fl = filter_str.to_lowercase();

        let online: Vec<&ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| r.alive).collect();
        let offline: Vec<&ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| !r.alive).collect();

        let online_filtered: Vec<&&ConsoleOnlineMinionRow> =
            online.iter().filter(|r| fl.is_empty() || Self::online_host(r).to_lowercase().contains(&fl)).collect();
        let offline_filtered: Vec<&&ConsoleOnlineMinionRow> =
            offline.iter().filter(|r| fl.is_empty() || Self::online_host(r).to_lowercase().contains(&fl)).collect();

        let n_online = online_filtered.len();
        let n_offline = offline_filtered.len();

        let overshadowed = self.minion_logs_visible || self.minion_traits_visible || self.minions_menu_visible;
        let border = if overshadowed { palette::PROCESSING_BASE_DIMMED } else { palette::PROCESSING_BASE };
        let (bg_base, bg_glow, bg_heat, bg_peak, fg_dim) = if overshadowed {
            (
                palette::PROCESSING_BASE_DIMMED,
                palette::PROCESSING_GLOW_DIMMED,
                palette::PROCESSING_HEAT_DIMMED,
                palette::PROCESSING_PEAK_DIMMED,
                palette::MUTED,
            )
        } else {
            (palette::PROCESSING_BASE, palette::PROCESSING_GLOW, palette::PROCESSING_HEAT, palette::PROCESSING_PEAK, palette::FG)
        };
        let focus_enabled = !overshadowed;

        let popup_bg = palette::BG_1;

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(popup_bg));

        let inner = block.inner(parent);
        Clear.render(parent, buf);
        block.render(parent, buf);

        let title_style = TitleStyle::cyberpunk(border);
        title::overlay_gradient_title(
            buf,
            parent,
            &title_style,
            &[
                TitleSegment { text: " Minions ".into(), bg: bg_base, fg: fg_dim, modifier: Modifier::empty() },
                TitleSegment { text: format!(" {n_online} online "), bg: bg_glow, fg: palette::SUCCESS, modifier: Modifier::empty() },
                TitleSegment { text: format!(" {n_offline} offline "), bg: bg_heat, fg: palette::WARNING, modifier: Modifier::empty() },
                TitleSegment { text: format!(" {} total ", n_online + n_offline), bg: bg_peak, fg: fg_dim, modifier: Modifier::empty() },
            ],
        );

        if inner.height < 4 {
            return;
        }

        let [filter_area, panes_area]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();

        Self::_render_filter(filter_area, buf, focus_enabled && self.minions_focus == 0, &self.minions_filter_input);

        let [online_pane, offline_pane]: [Rect; 2] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(panes_area)
            .as_ref()
            .try_into()
            .unwrap();

        let max_w = 10u16;

        let online_selected = self.minions_online_sel.min(online_filtered.len().saturating_sub(1));
        let offline_selected = self.minions_offline_sel.min(offline_filtered.len().saturating_sub(1));

        Self::_render_pane(self, "Online", &online_filtered, online_pane, buf, focus_enabled && self.minions_focus == 1, online_selected, max_w);
        Self::_render_pane(self, "Offline", &offline_filtered, offline_pane, buf, focus_enabled && self.minions_focus == 2, offline_selected, max_w);
    }

    fn _render_filter(area: Rect, buf: &mut Buffer, focused: bool, filter_state: &InputState) {
        let label_style =
            if focused { Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD) } else { Style::default().fg(palette::MUTED) };
        buf.set_string(area.x, area.y, "Filter: ", label_style);

        let input_x = area.x + 8u16;
        let input_w = area.width.saturating_sub(8);
        if input_w == 0 {
            return;
        }

        let mut is = InputState::new();
        is.set_value(filter_state.value().to_string());
        is.set_focused(focused);
        let fc = filter_state.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        let inp = Input::new("").prompt("").placeholder("type to filter...");
        StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }

    #[allow(clippy::too_many_arguments)]
    fn _render_pane(
        &self, title: &str, filtered: &[&&ConsoleOnlineMinionRow], area: Rect, buf: &mut Buffer, active: bool, selected: usize, max_w: u16,
    ) {
        let popup_bg = palette::BG_1;
        let t = format!(" {title} ({}) ", filtered.len());
        let block = if active {
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("\u{E0B2}", Style::default().fg(palette::ACCENT)),
                    Span::styled(t, Style::default().fg(palette::BLACK).bg(palette::ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled("\u{E0B0}", Style::default().fg(palette::ACCENT)),
                ]))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::ACCENT))
        } else {
            Block::default()
                .borders(Borders::ALL)
                .title(t)
                .title_style(Style::default().fg(palette::MUTED))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::FAINT))
        };

        let inner = block.inner(area);
        block.render(area, buf);

        if filtered.is_empty() {
            let label = if title == "Online" { "No online minions" } else { "No offline minions" };
            let v_pad = inner.height.saturating_sub(1) / 2;
            let centered = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(v_pad), Constraint::Length(1), Constraint::Min(0)])
                .split(inner);
            Paragraph::new(label).alignment(Alignment::Center).style(Style::default().fg(palette::MUTED).bg(popup_bg)).render(centered[1], buf);
            return;
        }

        let ip_data: Vec<String> = filtered.iter().map(|r| Self::_fmt_ip(&r.ip)).collect();
        let host_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&Self::online_host(r), max_w as usize)).collect();
        let ver_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&Self::_fmt_version(r), max_w as usize)).collect();
        let id_data: Vec<String> = filtered.iter().map(|r| Self::shorten_mid(&r.minion_id, 4)).collect();
        let os_data: Vec<String> = filtered
            .iter()
            .map(|r| {
                let name = if r.os_name.is_empty() { "-" } else { r.os_name.as_str() };
                let dist = if r.os_distribution.is_empty() { "-" } else { r.os_distribution.as_str() };
                Self::_trunc_ellipsis(&format!("{name}/{dist}"), max_w as usize)
            })
            .collect();
        let osv_data: Vec<String> = filtered.iter().map(|r| Self::_trunc_ellipsis(&r.os_version, max_w as usize)).collect();
        let ker_data: Vec<String> = filtered.iter().map(|r| r.kernel.clone()).collect();

        let ip_w = ip_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
        let host_w = host_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(4).max(4);
        let ver_w = ver_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(7).min(max_w);
        let id_w = id_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
        let os_w = os_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
        let osv_w = osv_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);
        let ker_w = ker_data.iter().map(|s| s.chars().count() as u16).max().unwrap_or(2).max(2);

        let base_w: Vec<u16> = vec![ip_w, host_w, ver_w, id_w, os_w, osv_w, ker_w];
        let mut cols: Vec<Constraint> = base_w.into_iter().map(Constraint::Length).collect();
        cols.push(Constraint::Min(1));

        let sel_style = if active { Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT) } else { Style::default().fg(palette::SECONDARY) };
        let norm_style = Style::default().fg(palette::FG).bg(popup_bg);
        let rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let sty = if idx == selected { sel_style } else { norm_style };
                Row::new(vec![
                    ip_data[idx].as_str(),
                    host_data[idx].as_str(),
                    ver_data[idx].as_str(),
                    id_data[idx].as_str(),
                    os_data[idx].as_str(),
                    osv_data[idx].as_str(),
                    ker_data[idx].as_str(),
                    "",
                ])
                .style(sty)
            })
            .collect();

        Self::_render_scrollable_table(inner, buf, &cols, &rows, selected);
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

    pub(crate) fn _info_value_str(value: &Value) -> String {
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

    pub(crate) fn _render_scrollable_table(area: Rect, buf: &mut Buffer, cols: &[Constraint], rows: &[Row], selected: usize) {
        let vis_rows = area.height as usize;
        let start = if selected < vis_rows { 0 } else { (selected + 1).saturating_sub(vis_rows) };
        let end = (start + vis_rows).min(rows.len());
        let displayed: Vec<Row> = if start < rows.len() { rows[start..end].to_vec() } else { vec![] };
        Widget::render(Table::new(displayed, cols).column_spacing(2), area, buf);
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
