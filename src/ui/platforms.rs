use super::palette;
use super::title::{self, TitleSegment, TitleStyle};
use crossterm::event::KeyCode;
use libsysinspect::traits::os_display_name;
use ratatui::widgets::StatefulWidget;
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use ratatui_cheese::input::{Input, InputState, InputStyles};
use ratatui_glamour::color::blend_2d;
use std::cell::Cell;

#[derive(Debug)]
pub struct PlatformRow {
    pub platform: String,
    pub arch: String,
    pub version: String,
    pub size: String,
    pub checksum: String,
}

#[derive(Debug)]
pub struct PlatformsManager {
    pub rows: Vec<PlatformRow>,
    pub cursor: usize,
    pub scroll: Cell<usize>,

    pub delete_visible: bool,
    pub delete_name: String,
    pub delete_focus: DeleteFocus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteFocus {
    YesBtn,
    NoBtn,
}

impl Default for PlatformsManager {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            cursor: 0,
            scroll: Cell::new(0),
            delete_visible: false,
            delete_name: String::new(),
            delete_focus: DeleteFocus::YesBtn,
        }
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

    pub fn open_delete(&mut self, name: String) {
        self.delete_name = name;
        self.delete_focus = DeleteFocus::YesBtn;
        self.delete_visible = true;
    }

    pub fn handle_delete_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => {
                self.delete_visible = false;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.delete_focus = match self.delete_focus {
                    DeleteFocus::YesBtn => DeleteFocus::NoBtn,
                    DeleteFocus::NoBtn => DeleteFocus::YesBtn,
                };
            }
            KeyCode::Enter => return false,
            _ => {}
        }
        true
    }

    pub fn render_delete(&self, parent: Rect, buf: &mut Buffer) {
        let w = (parent.width / 2).clamp(40, 60);
        let h: u16 = 6;
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_1, palette::BG_0] as &[Color]);
        for ry in 0..canvas.height {
            for cx in 0..canvas.width {
                let idx = ry as usize * canvas.width as usize + cx as usize;
                if let Some(cell) = buf.cell_mut(Position::new(canvas.x + cx, canvas.y + ry)) {
                    cell.set_bg(grad[idx]);
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::PROCESSING_GLOW))
            .style(Style::default());
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        title::overlay_gradient_title(
            buf,
            canvas,
            &title_style,
            &[TitleSegment { text: format!(" Delete {} ", self.delete_name), bg: palette::ERROR_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        let msg = format!("Delete platform \"{}\"?", self.delete_name);
        let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
        buf.set_string(x, inner.y + 1, &msg, Style::default().fg(palette::FG));

        let btn_y = inner.y + 3;
        let yes_lbl = "[   Yes   ]";
        let no_lbl = "[   No    ]";
        let yes_w: u16 = 10;
        let no_w: u16 = 10;
        let gap: u16 = 3;
        let total_btn_w = yes_w + gap + no_w;
        let btn_x = inner.x + (inner.width.saturating_sub(total_btn_w)) / 2;

        let sel_btn = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let unsel_btn = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let yes_style = if self.delete_focus == DeleteFocus::YesBtn { sel_btn } else { unsel_btn };
        let no_style = if self.delete_focus == DeleteFocus::NoBtn { sel_btn } else { unsel_btn };
        buf.set_string(btn_x, btn_y, yes_lbl, yes_style);
        buf.set_string(btn_x + yes_w + gap, btn_y, no_lbl, no_style);

        Self::draw_shadow(buf, canvas, w, h);
    }

    fn draw_shadow(buf: &mut Buffer, canvas: Rect, dlg_w: u16, dlg_h: u16) {
        let buf_area = buf.area();
        let x = canvas.x;
        let y = canvas.y;
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);
        for idx in 0..dlg_w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(dlg_h);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }
        for offset in 0..2u16 {
            for idx in 0..dlg_h {
                let sx = x.saturating_add(dlg_w).saturating_add(offset);
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

        let hl = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);
        let plat_w: u16 = 12;
        let arch_w: u16 = 10;
        let ver_w: u16 = 10;
        let size_w: u16 = 10;
        let sum_w = list_area.width.saturating_sub(plat_w + arch_w + ver_w + size_w + 16);

        for i in 0..view_h.min(total.saturating_sub(s)) {
            let fi = s + i;
            let (_oi, row) = filtered[fi];
            let ry = list_area.y + i as u16;
            let sel = !filter_focus && fi == cursor;
            let display_platform = platform_label(&row.platform);
            let row_style = if sel { hl } else { Style::default().fg(palette::FG) };
            if sel {
                for cx in 0..list_area.width {
                    if let Some(cell) = buf.cell_mut(Position::new(list_area.x + cx, ry)) {
                        cell.set_bg(palette::HIGHLIGHT);
                    }
                }
            }
            buf.set_string(list_area.x + 1, ry, format!(" {}", truncate_str(&display_platform, plat_w as usize)), row_style);
            buf.set_string(list_area.x + 1 + plat_w + 1, ry, format!(" {}", truncate_str(&row.arch, arch_w as usize)), row_style);
            buf.set_string(list_area.x + 1 + plat_w + 1 + arch_w + 1, ry, format!(" {}", truncate_str(&row.version, ver_w as usize)), row_style);
            buf.set_string(
                list_area.x + 1 + plat_w + 1 + arch_w + 1 + ver_w + 1,
                ry,
                format!(" {}", truncate_str(&row.size, size_w as usize)),
                row_style,
            );
            let sum_x = list_area.x + 1 + plat_w + 1 + arch_w + 1 + ver_w + 1 + size_w + 1;
            buf.set_string(sum_x, ry, format!(" {}", truncate_str(&row.checksum, sum_w as usize)), row_style);
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

fn platform_label(raw: &str) -> String {
    let normalized = raw.to_lowercase();
    os_display_name(&normalized).to_string()
}

fn truncate_str(s: &str, max_w: usize) -> String {
    if s.len() <= max_w { s.to_string() } else { format!("{}…", &s[..max_w.saturating_sub(1)]) }
}
