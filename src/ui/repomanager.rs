use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
};
use libsysinspect::console::ConsoleModuleRow;
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use ratatui_glamour::color::blend_2d;
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct StagedModule {
    pub name: String,
    pub version: Option<String>,
    pub descr: String,
    pub path: std::path::PathBuf,
    pub checked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagingFocus {
    List,
    AddSelected,
    Cancel,
}

#[derive(Debug)]
pub struct RepoManager {
    pub visible: bool,
    pub rows: Vec<ConsoleModuleRow>,
    pub cursor: usize,
    pub scroll: Cell<usize>,

    // Staging
    pub staging: bool,
    pub staged: Vec<StagedModule>,
    pub staging_cursor: usize,
    pub staging_scroll: Cell<usize>,
    pub staging_focus: StagingFocus,
}

impl Default for RepoManager {
    fn default() -> Self {
        Self {
            visible: false,
            rows: Vec::new(),
            cursor: 0,
            scroll: Cell::new(0),
            staging: false,
            staged: Vec::new(),
            staging_cursor: 0,
            staging_scroll: Cell::new(0),
            staging_focus: StagingFocus::List,
        }
    }
}

impl RepoManager {
    pub fn enter_staging(&mut self, modules: Vec<StagedModule>) {
        self.staged = modules;
        self.staging_cursor = 0;
        self.staging_scroll = Cell::new(0);
        self.staging_focus = StagingFocus::List;
        self.staging = true;
    }

    pub fn exit_staging(&mut self) {
        self.staging = false;
        self.staged.clear();
    }

    pub fn handle_staging_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        if !self.staging {
            return false;
        }
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.exit_staging();
            }
            crossterm::event::KeyCode::Tab => {
                use StagingFocus::*;
                self.staging_focus = match self.staging_focus {
                    List => AddSelected,
                    AddSelected => Cancel,
                    Cancel => List,
                };
            }
            crossterm::event::KeyCode::BackTab => {
                use StagingFocus::*;
                self.staging_focus = match self.staging_focus {
                    List => Cancel,
                    AddSelected => List,
                    Cancel => AddSelected,
                };
            }
            crossterm::event::KeyCode::Up if self.staging_focus == StagingFocus::List => {
                self.staging_cursor = self.staging_cursor.saturating_sub(1);
            }
            crossterm::event::KeyCode::Down if self.staging_focus == StagingFocus::List => {
                self.staging_cursor = (self.staging_cursor + 1).min(self.staged.len().saturating_sub(1));
            }
            crossterm::event::KeyCode::Char(' ') if self.staging_focus == StagingFocus::List => {
                if let Some(m) = self.staged.get_mut(self.staging_cursor) {
                    m.checked = !m.checked;
                }
            }
            crossterm::event::KeyCode::Enter => match self.staging_focus {
                StagingFocus::AddSelected => {
                    // TODO: bulk register checked modules
                    self.exit_staging();
                }
                StagingFocus::Cancel => {
                    self.exit_staging();
                }
                _ => {}
            },
            _ => {}
        }
        true
    }

    pub fn render(&self, parent: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }
        if self.staging {
            self.render_staging(parent, buf);
        } else {
            self.render_main(parent, buf);
        }
    }

    fn render_main(&self, parent: Rect, buf: &mut Buffer) {
        let dlg_w = (parent.width * 3 / 4).clamp(70, 110);
        let dlg_h = (parent.height * 3 / 4).clamp(10, 24);
        let x = parent.x + (parent.width.saturating_sub(dlg_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(dlg_h)) / 2;
        let canvas = Rect { x, y, width: dlg_w, height: dlg_h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::GRAY_0, palette::BG_2] as &[Color]);
        for row in 0..canvas.height {
            for col in 0..canvas.width {
                let idx = row as usize * canvas.width as usize + col as usize;
                if let Some(cell) = buf.cell_mut(Position::new(canvas.x + col, canvas.y + row)) {
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
            &[TitleSegment { text: " Module and Library Manager ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG }],
        );

        if inner.height >= 3 && !self.rows.is_empty() {
            let name_w: u16 = 28;
            let ver_w: u16 = 6;
            let desc_w = inner.width.saturating_sub(name_w + ver_w + 2);

            let view_h = inner.height as usize;
            let total = self.rows.len();
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

            let muted = Style::default().fg(palette::MUTED);
            let hl = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);
            let dim_hl = Style::default().fg(palette::BG_0).bg(palette::HIGHLIGHT);

            for i in 0..view_h.min(total.saturating_sub(s)) {
                let idx = s + i;
                let ry = inner.y + i as u16;
                let row = &self.rows[idx];
                let selected = idx == cursor;
                let row_style = if selected { hl } else { Style::default().fg(palette::FG) };
                let dim = if selected { dim_hl } else { muted };

                let name = truncate_str(&row.name, name_w as usize);
                buf.set_string(inner.x + 1, ry, format!(" {name}"), row_style);
                let ver = row.version.as_deref().unwrap_or("—");
                buf.set_string(inner.x + 1 + name_w + 1, ry, truncate_str(ver, ver_w as usize), dim);
                let desc_x = inner.x + 1 + name_w + 1 + ver_w + 1;
                let max_desc = desc_w.saturating_sub(1) as usize;
                buf.set_string(desc_x, ry, truncate_str(&row.descr, max_desc), dim);
            }

            if total > view_h {
                let bar_h = ((view_h as f64 / total as f64) * view_h as f64).max(1.0) as usize;
                let bar_y = ((s as f64 / total as f64) * (view_h - bar_h) as f64) as usize;
                for i in 0..view_h {
                    let sx = inner.right().saturating_sub(1);
                    let sy = inner.y + i as u16;
                    if i >= bar_y && i < bar_y + bar_h {
                        buf.set_string(sx, sy, "█", Style::default().fg(palette::PROCESSING_HEAT));
                    } else {
                        buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
                    }
                }
            }
        } else if inner.height >= 3 {
            let msg = "(no modules found)";
            let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
            let y = inner.y + inner.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
        }

        Self::draw_shadow(buf, canvas, dlg_w, dlg_h);
    }

    fn render_staging(&self, parent: Rect, buf: &mut Buffer) {
        let dlg_w = (parent.width * 3 / 4).clamp(70, 110);
        let module_rows = self.staged.len().min(20) as u16;
        let btn_height: u16 = 2;
        let dlg_h = (module_rows + btn_height + 2).clamp(8, parent.height * 3 / 4);
        let x = parent.x + (parent.width.saturating_sub(dlg_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(dlg_h)) / 2;
        let canvas = Rect { x, y, width: dlg_w, height: dlg_h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::GRAY_0, palette::BG_2] as &[Color]);
        for row in 0..canvas.height {
            for col in 0..canvas.width {
                let idx = row as usize * canvas.width as usize + col as usize;
                if let Some(cell) = buf.cell_mut(Position::new(canvas.x + col, canvas.y + row)) {
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
            &[TitleSegment { text: " Module and Library Manager ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG }],
        );

        if inner.height < 6 || self.staged.is_empty() {
            return;
        }

        let list_height = inner.height.saturating_sub(btn_height);

        let name_w: u16 = 28;
        let ver_w: u16 = 6;

        let view_h = list_height as usize;
        let total = self.staged.len();
        let max_scroll = total.saturating_sub(view_h);
        let mut s = self.staging_scroll.get();
        let cursor = self.staging_cursor.min(total.saturating_sub(1));
        if cursor < s {
            s = cursor;
        }
        if cursor >= s + view_h {
            s = cursor.saturating_sub(view_h.saturating_sub(1));
        }
        s = s.min(max_scroll);
        self.staging_scroll.set(s);

        let hl = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);

        for i in 0..view_h.min(total.saturating_sub(s)) {
            let idx = s + i;
            let ry = inner.y + i as u16;
            let m = &self.staged[idx];
            let sel = idx == cursor && self.staging_focus == StagingFocus::List;
            let row_style = if sel { hl } else { Style::default().fg(palette::FG) };

            // Fill entire row with highlight background when selected
            if sel {
                for cx in 0..inner.width {
                    if let Some(cell) = buf.cell_mut(Position::new(inner.x + cx, ry)) {
                        cell.set_bg(palette::HIGHLIGHT);
                    }
                }
            }

            let (check_ch, check_style) =
                if m.checked { ("▣", Style::default().fg(palette::SUCCESS)) } else { ("□", Style::default().fg(palette::GRAY_1)) };
            buf.set_string(inner.x + 1, ry, check_ch, if sel { row_style } else { check_style });

            let name = truncate_str(&m.name, name_w as usize);
            buf.set_string(inner.x + 5, ry, &name, row_style);

            let ver = m.version.as_deref().unwrap_or("—");
            let ver_style = if sel { row_style } else { Style::default().fg(palette::HIGHLIGHT) };
            buf.set_string(inner.x + 5 + name_w + 1, ry, truncate_str(ver, ver_w as usize), ver_style);

            let desc_x = inner.x + 5 + name_w + 1 + ver_w + 1;
            let max_desc = inner.width.saturating_sub(5 + name_w + ver_w + 4) as usize;
            let desc_style = if sel { row_style } else { Style::default().fg(palette::GRAY_1) };
            buf.set_string(desc_x, ry, truncate_str(&m.descr, max_desc), desc_style);
        }

        if total > view_h {
            let bar_h = ((view_h as f64 / total as f64) * view_h as f64).max(1.0) as usize;
            let bar_y = ((s as f64 / total as f64) * (view_h - bar_h) as f64) as usize;
            for i in 0..view_h {
                let sx = inner.right().saturating_sub(1);
                let sy = inner.y + i as u16;
                if i >= bar_y && i < bar_y + bar_h {
                    buf.set_string(sx, sy, "█", Style::default().fg(palette::PROCESSING_HEAT));
                } else {
                    buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
                }
            }
        }

        // Buttons
        let btn_y = inner.y + list_height + 1;
        let add_label = "[ Add Selected ]";
        let cancel_label = "[ Cancel ]";
        let add_w = add_label.len() as u16;
        let cancel_w = cancel_label.len() as u16;
        let total_btn_w = add_w + cancel_w + 6;
        let btn_x = inner.x + (inner.width.saturating_sub(total_btn_w)) / 2;

        let sel_btn = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let unsel_btn = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let add_style = if self.staging_focus == StagingFocus::AddSelected { sel_btn } else { unsel_btn };
        let cancel_style = if self.staging_focus == StagingFocus::Cancel { sel_btn } else { unsel_btn };

        buf.set_string(btn_x, btn_y, add_label, add_style);
        buf.set_string(btn_x + add_w + 4, btn_y, cancel_label, cancel_style);

        Self::draw_shadow(buf, canvas, dlg_w, dlg_h);
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
}

fn truncate_str(s: &str, max_w: usize) -> String {
    if s.len() <= max_w { s.to_string() } else { format!("{}…", &s[..max_w.saturating_sub(1)]) }
}
