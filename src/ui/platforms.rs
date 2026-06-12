use super::palette;
use crossterm::event::KeyCode;
use ratatui::widgets::StatefulWidget;
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
};
use ratatui_cheese::input::{Input, InputState, InputStyles};
use std::cell::Cell;

#[derive(Debug)]
pub struct PlatformRow {
    pub platform: String,
    pub arch: String,
    pub version: String,
    pub checksum: String,
}

#[derive(Debug)]
pub struct PlatformsManager {
    pub rows: Vec<PlatformRow>,
    pub cursor: usize,
    pub scroll: Cell<usize>,
}

impl Default for PlatformsManager {
    fn default() -> Self {
        Self { rows: Vec::new(), cursor: 0, scroll: Cell::new(0) }
    }
}

impl PlatformsManager {
    pub fn handle_list_key(&mut self, key: KeyCode) -> bool {
        let page = 10usize;
        let max = self.rows.len().saturating_sub(1);
        match key {
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => self.cursor = (self.cursor + 1).min(max),
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(page),
            KeyCode::PageDown => self.cursor = (self.cursor + page).min(max),
            _ => return false,
        }
        true
    }

    pub fn filtered_count(&self, filter_value: &str) -> usize {
        let f = filter_value.to_lowercase();
        if f.is_empty() {
            return self.rows.len();
        }
        self.rows
            .iter()
            .filter(|r| r.platform.to_lowercase().contains(&f) || r.arch.to_lowercase().contains(&f) || r.version.to_lowercase().contains(&f))
            .count()
    }

    pub fn selected_name(&self) -> Option<String> {
        self.rows.get(self.cursor).map(|r| format!("{}/{}", r.platform, r.arch))
    }

    pub fn render_list(&self, inner: Rect, buf: &mut Buffer, filter_focus: bool, filter_state: &InputState) {
        if inner.height < 2 {
            return;
        }

        let [filter_area, list_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([ratatui::layout::Constraint::Length(1), ratatui::layout::Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();

        Self::render_filter_row(filter_area, buf, filter_focus, filter_state);

        if self.rows.is_empty() {
            let msg = "(no platform builds found)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }

        let flt = filter_state.value().to_lowercase();
        let filtered: Vec<(usize, &PlatformRow)> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                flt.is_empty()
                    || r.platform.to_lowercase().contains(&flt)
                    || r.arch.to_lowercase().contains(&flt)
                    || r.version.to_lowercase().contains(&flt)
            })
            .collect();

        let view_h = list_area.height as usize;
        let total = filtered.len();
        let max_scroll = total.saturating_sub(view_h);
        let mut s = self.scroll.get();
        let cursor = self.cursor.min(total.saturating_sub(1));
        if cursor < s {
            s = cursor;
        }
        if cursor >= s + view_h {
            s = cursor.saturating_sub(view_h.saturating_sub(1));
        }
        s = s.min(max_scroll);
        self.scroll.set(s);

        if total == 0 {
            let msg = "(no matches)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }

        let hl_style = Style::default().fg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD);
        let plat_w: u16 = 12;
        let arch_w: u16 = 10;
        let ver_w: u16 = 10;
        let sum_w = list_area.width.saturating_sub(plat_w + arch_w + ver_w + 12);

        for i in 0..view_h.min(total.saturating_sub(s)) {
            let fi = s + i;
            let (_oi, row) = filtered[fi];
            let ry = list_area.y + i as u16;
            let sel = !filter_focus && fi == cursor;
            let row_style = if sel { hl_style } else { Style::default().fg(palette::FG) };
            let prefix = if sel { " ✨ " } else { "    " };
            buf.set_string(list_area.x + 1, ry, prefix, row_style);
            buf.set_string(list_area.x + 5, ry, truncate_str(&row.platform, plat_w as usize), row_style);
            let arch_style = if sel { row_style } else { Style::default().fg(palette::PROCESSING) };
            buf.set_string(list_area.x + 5 + plat_w + 1, ry, truncate_str(&row.arch, arch_w as usize), arch_style);
            let ver_style = if sel { row_style } else { Style::default().fg(palette::HIGHLIGHT) };
            buf.set_string(list_area.x + 5 + plat_w + 1 + arch_w + 1, ry, truncate_str(&row.version, ver_w as usize), ver_style);
            let sum_style = if sel { row_style } else { Style::default().fg(palette::GRAY_1) };
            let sum_x = list_area.x + 5 + plat_w + 1 + arch_w + 1 + ver_w + 1;
            buf.set_string(sum_x, ry, truncate_str(&row.checksum, sum_w as usize), sum_style);
        }

        if total > view_h {
            let bh = ((view_h as f64 / total as f64) * view_h as f64).max(1.0) as usize;
            let by = ((s as f64 / total as f64) * (view_h - bh) as f64) as usize;
            for i in 0..view_h {
                let sx = list_area.right().saturating_sub(1);
                let sy = list_area.y + i as u16;
                if i >= by && i < by + bh {
                    buf.set_string(sx, sy, "█", Style::default().fg(palette::PROCESSING_HEAT));
                } else {
                    buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
                }
            }
        }
    }

    fn render_filter_row(area: Rect, buf: &mut Buffer, focused: bool, filter_state: &InputState) {
        let label_style = if focused { Style::default().fg(palette::ACCENT) } else { Style::default().fg(palette::MUTED) };
        buf.set_string(area.x + 2, area.y, "filter: ", label_style);

        let input_x = area.x + 10;
        let input_w = area.width.saturating_sub(10);
        if input_w == 0 {
            return;
        }

        let field_bg = if focused { palette::HIGHLIGHT } else { palette::GRAY_1 };
        for cx in input_x..input_x + input_w {
            if let Some(cell) = buf.cell_mut(Position::new(cx, area.y)) {
                cell.set_bg(field_bg);
            }
        }

        let mut is = InputState::new();
        is.set_value(filter_state.value().to_string());
        is.set_focused(focused);
        let fc = filter_state.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        let styles = InputStyles { text: Style::default().fg(palette::BG_1), ..Default::default() };
        let inp = Input::new("").prompt("").placeholder("search platforms...").styles(styles);
        StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }
}

fn truncate_str(s: &str, max_w: usize) -> String {
    if s.len() <= max_w { s.to_string() } else { format!("{}…", &s[..max_w.saturating_sub(1)]) }
}
