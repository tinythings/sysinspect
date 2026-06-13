use super::palette;
use super::title::{self, TitleSegment, TitleStyle};
use crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::prelude::Buffer;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use ratatui_cheese::input::InputState;
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::dashed_title;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfDetailFocus {
    Modules,
    Libraries,
    AddModuleBtn,
    AddLibraryBtn,
    CloseBtn,
}

impl ProfDetailFocus {
    pub fn next(self, has_modules: bool, has_libraries: bool) -> Self {
        use ProfDetailFocus::*;
        let mut cur = self;
        loop {
            cur = match cur {
                Modules => Libraries,
                Libraries => AddModuleBtn,
                AddModuleBtn => AddLibraryBtn,
                AddLibraryBtn => CloseBtn,
                CloseBtn => Modules,
            };
            match cur {
                Modules if !has_modules => continue,
                Libraries if !has_libraries => continue,
                _ => return cur,
            }
        }
    }

    pub fn prev(self, has_modules: bool, has_libraries: bool) -> Self {
        use ProfDetailFocus::*;
        let mut cur = self;
        loop {
            cur = match cur {
                Modules => CloseBtn,
                Libraries => Modules,
                AddModuleBtn => Libraries,
                AddLibraryBtn => AddModuleBtn,
                CloseBtn => AddLibraryBtn,
            };
            match cur {
                Modules if !has_modules => continue,
                Libraries if !has_libraries => continue,
                _ => return cur,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfCreateFocus {
    Input,
    CreateBtn,
    CancelBtn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfDeleteFocus {
    YesBtn,
    NoBtn,
}

#[derive(Debug)]
pub struct ResolvedModule {
    pub name: String,
    pub version: String,
    pub descr: String,
    pub selector: String,
}

#[derive(Debug)]
pub struct ResolvedLibrary {
    pub name: String,
    pub kind: String,
    pub checksum: String,
    pub selector: String,
}

#[derive(Debug)]
pub struct ProfilesManager {
    // List
    pub profiles: Vec<String>,
    pub cursor: usize,
    pub scroll: Cell<usize>,

    // Detail overlay
    pub detail_visible: bool,
    pub detail_name: String,
    pub detail_modules: Vec<ResolvedModule>,
    pub detail_libraries: Vec<ResolvedLibrary>,
    pub detail_focus: ProfDetailFocus,
    pub detail_moffset: Cell<usize>,
    pub detail_loffset: Cell<usize>,

    // Create overlay
    pub create_visible: bool,
    pub create_input: InputState,
    pub create_focus: ProfCreateFocus,

    // Delete overlay
    pub delete_visible: bool,
    pub delete_name: String,
    pub delete_focus: ProfDeleteFocus,
}

impl Default for ProfilesManager {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            cursor: 0,
            scroll: Cell::new(0),
            detail_visible: false,
            detail_name: String::new(),
            detail_modules: Vec::new(),
            detail_libraries: Vec::new(),
            detail_focus: ProfDetailFocus::Modules,
            detail_moffset: Cell::new(0),
            detail_loffset: Cell::new(0),
            create_visible: false,
            create_input: InputState::new(),
            create_focus: ProfCreateFocus::Input,
            delete_visible: false,
            delete_name: String::new(),
            delete_focus: ProfDeleteFocus::YesBtn,
        }
    }
}

impl ProfilesManager {
    // ── List key handling ──

    pub fn handle_list_key(&mut self, key: KeyCode, filter_focus: &mut bool, filter_value: &str) -> bool {
        let page = 10usize;
        let total = self.filtered_count(filter_value);
        let max = total.saturating_sub(1);
        match key {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(max);
            }
            KeyCode::PageUp => {
                self.cursor = self.cursor.saturating_sub(page);
            }
            KeyCode::PageDown => {
                self.cursor = (self.cursor + page).min(max);
            }
            KeyCode::Tab => {
                *filter_focus = true;
            }
            KeyCode::Char('/') => {
                *filter_focus = true;
            }
            _ => return false,
        }
        true
    }

    pub fn filtered_count(&self, filter_value: &str) -> usize {
        let f = filter_value.to_lowercase();
        if f.is_empty() {
            return self.profiles.len();
        }
        self.profiles.iter().filter(|n| n.to_lowercase().contains(&f)).count()
    }

    // ── Detail key handling ──

    pub fn handle_detail_key(&mut self, key: KeyCode) -> bool {
        use ProfDetailFocus::*;
        match key {
            KeyCode::Esc => {
                self.detail_visible = false;
            }
            KeyCode::Tab => {
                let hm = !self.detail_modules.is_empty();
                let hl = !self.detail_libraries.is_empty();
                self.detail_focus = self.detail_focus.next(hm, hl);
            }
            KeyCode::BackTab => {
                let hm = !self.detail_modules.is_empty();
                let hl = !self.detail_libraries.is_empty();
                self.detail_focus = self.detail_focus.prev(hm, hl);
            }
            KeyCode::Up => match self.detail_focus {
                Modules => {
                    let o = self.detail_moffset.get();
                    self.detail_moffset.set(o.saturating_sub(1));
                }
                Libraries => {
                    let o = self.detail_loffset.get();
                    self.detail_loffset.set(o.saturating_sub(1));
                }
                _ => {}
            },
            KeyCode::Down => match self.detail_focus {
                Modules => {
                    let o = self.detail_moffset.get();
                    let view_h = 10usize; // approximate, clamped in render
                    let max = self.detail_modules.len().saturating_sub(view_h);
                    self.detail_moffset.set((o + 1).min(max));
                }
                Libraries => {
                    let o = self.detail_loffset.get();
                    let view_h = 10usize;
                    let max = self.detail_libraries.len().saturating_sub(view_h);
                    self.detail_loffset.set((o + 1).min(max));
                }
                _ => {}
            },
            KeyCode::PageUp => match self.detail_focus {
                Modules => {
                    let o = self.detail_moffset.get();
                    self.detail_moffset.set(o.saturating_sub(10));
                }
                Libraries => {
                    let o = self.detail_loffset.get();
                    self.detail_loffset.set(o.saturating_sub(10));
                }
                _ => {}
            },
            KeyCode::PageDown => match self.detail_focus {
                Modules => {
                    let o = self.detail_moffset.get();
                    let max = self.detail_modules.len().saturating_sub(10);
                    self.detail_moffset.set((o + 10).min(max));
                }
                Libraries => {
                    let o = self.detail_loffset.get();
                    let max = self.detail_libraries.len().saturating_sub(10);
                    self.detail_loffset.set((o + 10).min(max));
                }
                _ => {}
            },
            KeyCode::Enter => return false,
            _ => {}
        }
        true
    }

    // ── Create key handling ──

    pub fn handle_create_key(&mut self, key: KeyCode) -> bool {
        use ProfCreateFocus::*;
        match key {
            KeyCode::Esc => {
                self.create_visible = false;
            }
            KeyCode::Tab => {
                self.create_focus = match self.create_focus {
                    Input => CreateBtn,
                    CreateBtn => CancelBtn,
                    CancelBtn => Input,
                };
            }
            KeyCode::BackTab => {
                self.create_focus = match self.create_focus {
                    Input => CancelBtn,
                    CreateBtn => Input,
                    CancelBtn => CreateBtn,
                };
            }
            KeyCode::Enter => return false, // handled in mod.rs
            KeyCode::Backspace if self.create_focus == Input => {
                self.create_input.delete_before();
            }
            KeyCode::Delete if self.create_focus == Input => {
                self.create_input.delete_at();
            }
            KeyCode::Left if self.create_focus == Input => {
                self.create_input.move_left();
            }
            KeyCode::Right if self.create_focus == Input => {
                self.create_input.move_right();
            }
            KeyCode::Home if self.create_focus == Input => {
                self.create_input.home();
            }
            KeyCode::End if self.create_focus == Input => {
                self.create_input.end();
            }
            KeyCode::Char(c) if self.create_focus == Input => {
                self.create_input.insert_char(c);
            }
            _ => {}
        }
        true
    }

    // ── Delete key handling ──

    pub fn handle_delete_key(&mut self, key: KeyCode) -> bool {
        use ProfDeleteFocus::*;
        match key {
            KeyCode::Esc => {
                self.delete_visible = false;
            }
            KeyCode::Tab => {
                self.delete_focus = match self.delete_focus {
                    YesBtn => NoBtn,
                    NoBtn => YesBtn,
                };
            }
            KeyCode::BackTab => {
                self.delete_focus = match self.delete_focus {
                    YesBtn => NoBtn,
                    NoBtn => YesBtn,
                };
            }
            KeyCode::Enter => return false, // handled in mod.rs
            _ => {}
        }
        true
    }

    // ── State management ──

    pub fn enter_detail(&mut self, name: String, modules: Vec<ResolvedModule>, libraries: Vec<ResolvedLibrary>) {
        self.detail_name = name;
        self.detail_modules = modules;
        self.detail_libraries = libraries;
        self.detail_focus = if !self.detail_modules.is_empty() {
            ProfDetailFocus::Modules
        } else if !self.detail_libraries.is_empty() {
            ProfDetailFocus::Libraries
        } else {
            ProfDetailFocus::AddModuleBtn
        };
        self.detail_moffset.set(0);
        self.detail_loffset.set(0);
        self.detail_visible = true;
    }

    pub fn open_create(&mut self) {
        self.create_input = InputState::new();
        self.create_focus = ProfCreateFocus::Input;
        self.create_visible = true;
    }

    pub fn open_delete(&mut self, name: String) {
        self.delete_name = name;
        self.delete_focus = ProfDeleteFocus::YesBtn;
        self.delete_visible = true;
    }

    pub fn selected_profile_name(&self) -> Option<&str> {
        self.profiles.get(self.cursor).map(|s| s.as_str())
    }

    // ── Rendering ──

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

        if self.profiles.is_empty() {
            let msg = "(no profiles found)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }

        let flt = filter_state.value().to_lowercase();
        let filtered: Vec<(usize, &String)> =
            self.profiles.iter().enumerate().filter(|(_, n)| flt.is_empty() || n.to_lowercase().contains(&flt)).collect();

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
        for i in 0..view_h.min(total.saturating_sub(s)) {
            let fi = s + i;
            let (_oi, name) = filtered[fi];
            let ry = list_area.y + i as u16;
            let sel = !filter_focus && fi == cursor;
            let row_style = if sel { hl } else { Style::default().fg(palette::FG) };
            if sel {
                for cx in 0..list_area.width {
                    if let Some(cell) = buf.cell_mut(Position::new(list_area.x + cx, ry)) {
                        cell.set_bg(palette::HIGHLIGHT);
                    }
                }
            }
            buf.set_string(list_area.x + 1, ry, format!(" {}", name), row_style);
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

    pub fn render_detail(&self, parent: Rect, buf: &mut Buffer) {
        let w = (parent.width * 80 / 100).max(60).min(parent.width.saturating_sub(2));
        let h = (parent.height * 80 / 100).clamp(14, 26);
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
        let focus_text = match self.detail_focus {
            ProfDetailFocus::Modules => Some(" Modules "),
            ProfDetailFocus::Libraries => Some(" Libraries "),
            _ => None,
        };
        let mut title_segments = vec![
            TitleSegment { text: " Profile ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() },
            TitleSegment {
                text: format!(" {} ", self.detail_name),
                bg: palette::PROCESSING_GLOW,
                fg: palette::SUCCESS_PEAK,
                modifier: Modifier::BOLD,
            },
        ];
        if let Some(ft) = focus_text {
            title_segments.push(TitleSegment { text: ft.into(), bg: palette::PROCESSING_HEAT, fg: palette::FG, modifier: Modifier::empty() });
        }
        title::overlay_gradient_title(buf, canvas, &title_style, &title_segments);

        if inner.height < 6 {
            return;
        }

        let btn_height: u16 = 3;
        let content_height = inner.height.saturating_sub(btn_height);
        let mod_h = content_height / 2;
        let _lib_h = content_height.saturating_sub(mod_h);

        let mut row_y = inner.y;

        // ── Modules section ──
        dashed_title(
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
            buf,
            " Modules ",
            palette::PROCESSING,
            palette::PROCESSING_PEAK,
            palette::PROCESSING_DIMMED,
        );
        row_y += 1;

        if mod_h > 0 {
            let mod_area = Rect { x: inner.x, y: row_y, width: inner.width.saturating_sub(1), height: mod_h.saturating_sub(1) };
            self.render_resolved_modules(mod_area, buf, self.detail_focus == ProfDetailFocus::Modules);
            row_y = mod_area.bottom() + 1;
        }

        // ── Libraries section ──
        if row_y + 2 <= inner.bottom() {
            dashed_title(
                Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
                buf,
                " Libraries ",
                palette::PROCESSING,
                palette::PROCESSING_PEAK,
                palette::PROCESSING_DIMMED,
            );
            row_y += 1;

            let lib_area = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width.saturating_sub(1),
                height: (inner.bottom().saturating_sub(row_y)).saturating_sub(btn_height),
            };
            self.render_resolved_libraries(lib_area, buf, self.detail_focus == ProfDetailFocus::Libraries);
        }

        // ── Buttons ──
        let btn_y = inner.bottom().saturating_sub(2);
        let btn_labels = ["[ Add Module ]", "[ Add Library ]", "[ Close ]"];
        let btn_widths: Vec<u16> = btn_labels.iter().map(|l| l.len() as u16).collect();
        let total_btn_w: u16 = btn_widths.iter().sum::<u16>() + 4; // 2 gaps
        let mut btn_x = inner.x + (inner.width.saturating_sub(total_btn_w)) / 2;

        let focus_idx = match self.detail_focus {
            ProfDetailFocus::AddModuleBtn => 0,
            ProfDetailFocus::AddLibraryBtn => 1,
            ProfDetailFocus::CloseBtn => 2,
            _ => usize::MAX,
        };
        let sel_btn = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT);
        let unsel_btn = Style::default().fg(palette::FG).bg(palette::BG_2);

        for (i, label) in btn_labels.iter().enumerate() {
            let style = if i == focus_idx { sel_btn } else { unsel_btn };
            buf.set_string(btn_x, btn_y, *label, style);
            btn_x += btn_widths[i] + 2;
        }

        Self::draw_shadow(buf, canvas, w, h);
    }

    fn render_resolved_modules(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        if self.detail_modules.is_empty() {
            let msg = "(no modules in this profile)";
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            Self::draw_scrollbar(buf, area, 0, 1, area.height as usize, focused);
            return;
        }

        let name_w: u16 = 28;
        let ver_w: u16 = 6;
        let view_h = area.height as usize;
        let total = self.detail_modules.len();
        let max_scroll = total.saturating_sub(view_h);
        let s = self.detail_moffset.get().min(max_scroll);
        self.detail_moffset.set(s);

        for i in 0..view_h.min(total.saturating_sub(s)) {
            let idx = s + i;
            let ry = area.y + i as u16;
            let m = &self.detail_modules[idx];
            let fg = if focused { palette::FG } else { palette::MUTED };
            let ver_fg = if focused { palette::HIGHLIGHT } else { palette::MUTED };
            let desc_fg = if focused { palette::GRAY_1 } else { palette::MUTED };
            let row_style = Style::default().fg(fg);
            buf.set_string(area.x + 2, ry, truncate_str(&m.name, name_w as usize), row_style);
            buf.set_string(area.x + 2 + name_w + 1, ry, truncate_str(&m.version, ver_w as usize), Style::default().fg(ver_fg));
            let desc_x = area.x + 2 + name_w + 1 + ver_w + 1;
            let max_desc = (area.width.saturating_sub(2 + name_w + ver_w + 3)) as usize;
            buf.set_string(desc_x, ry, truncate_str(&m.descr, max_desc), Style::default().fg(desc_fg));
        }

        Self::draw_scrollbar(buf, area, s, total, view_h, focused);
    }

    fn render_resolved_libraries(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        if self.detail_libraries.is_empty() {
            let msg = "(no libraries in this profile)";
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            Self::draw_scrollbar(buf, area, 0, 1, area.height as usize, focused);
            return;
        }

        let kind_w: u16 = 8;
        let name_w = area.width.saturating_sub(kind_w + 40);
        let sum_w = 30u16;
        let view_h = area.height as usize;
        let total = self.detail_libraries.len();
        let max_scroll = total.saturating_sub(view_h);
        let s = self.detail_loffset.get().min(max_scroll);
        self.detail_loffset.set(s);

        for i in 0..view_h.min(total.saturating_sub(s)) {
            let idx = s + i;
            let ry = area.y + i as u16;
            let lib = &self.detail_libraries[idx];
            let fg = if focused { palette::FG } else { palette::MUTED };
            let kind_fg = if focused { palette::PROCESSING } else { palette::MUTED };
            let sum_fg = if focused { palette::GRAY_1 } else { palette::MUTED };
            let row_style = Style::default().fg(fg);
            buf.set_string(area.x + 2, ry, truncate_str(&lib.kind, kind_w as usize), Style::default().fg(kind_fg));
            buf.set_string(area.x + 2 + kind_w + 1, ry, truncate_str(&lib.name, name_w as usize), row_style);
            let sum_x = area.x + 2 + kind_w + 1 + name_w + 1;
            buf.set_string(sum_x, ry, truncate_str(&lib.checksum, sum_w as usize), Style::default().fg(sum_fg));
        }

        Self::draw_scrollbar(buf, area, s, total, view_h, focused);
    }

    pub fn render_create(&self, parent: Rect, buf: &mut Buffer) {
        let w = (parent.width / 2).clamp(40, 60);
        let h: u16 = 6;
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_1, palette::BG_2] as &[Color]);
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
            &[TitleSegment { text: " Create Profile ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        // Name input row
        let label_style =
            if self.create_focus == ProfCreateFocus::Input { Style::default().fg(palette::ACCENT) } else { Style::default().fg(palette::MUTED) };
        buf.set_string(inner.x + 2, inner.y + 1, "Name:", label_style);

        let input_x = inner.x + 8;
        let input_w = inner.width.saturating_sub(10);
        if input_w > 0 && self.create_focus == ProfCreateFocus::Input {
            let field_bg = palette::HIGHLIGHT;
            for cx in input_x..input_x + input_w {
                if let Some(cell) = buf.cell_mut(Position::new(cx, inner.y + 1)) {
                    cell.set_bg(field_bg);
                }
            }
        }

        let mut is = InputState::new();
        is.set_value(self.create_input.value().to_string());
        is.set_focused(self.create_focus == ProfCreateFocus::Input);
        let fc = self.create_input.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        let styles = ratatui_cheese::input::InputStyles { text: Style::default().fg(palette::BG_1), ..Default::default() };
        let inp = ratatui_cheese::input::Input::new("").prompt("").placeholder("profile name").styles(styles);
        ratatui::widgets::StatefulWidget::render(&inp, Rect::new(input_x, inner.y + 1, input_w, 1), buf, &mut is);

        // Buttons
        let btn_y = inner.y + 3;
        let create_lbl = "[ Create  ]";
        let cancel_lbl = "[ Cancel  ]";
        let create_w: u16 = 10;
        let cancel_w: u16 = 10;
        let gap: u16 = 3;
        let total_btn_w = create_w + gap + cancel_w;
        let btn_x = inner.x + (inner.width.saturating_sub(total_btn_w)) / 2;

        let sel_btn = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let unsel_btn = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let create_style = if self.create_focus == ProfCreateFocus::CreateBtn { sel_btn } else { unsel_btn };
        let cancel_style = if self.create_focus == ProfCreateFocus::CancelBtn { sel_btn } else { unsel_btn };
        buf.set_string(btn_x, btn_y, create_lbl, create_style);
        buf.set_string(btn_x + create_w + gap, btn_y, cancel_lbl, cancel_style);

        Self::draw_shadow(buf, canvas, w, h);
    }

    pub fn render_delete(&self, parent: Rect, buf: &mut Buffer) {
        let w = (parent.width / 2).clamp(40, 60);
        let h: u16 = 6;
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_1, palette::BG_2] as &[Color]);
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
            &[TitleSegment { text: " Delete Profile ".into(), bg: palette::ERROR_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        // Confirm text
        let msg = format!("Delete profile \"{}\"?", self.delete_name);
        let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
        buf.set_string(x, inner.y + 1, &msg, Style::default().fg(palette::FG));

        // Buttons
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

        let yes_style = if self.delete_focus == ProfDeleteFocus::YesBtn { sel_btn } else { unsel_btn };
        let no_style = if self.delete_focus == ProfDeleteFocus::NoBtn { sel_btn } else { unsel_btn };
        buf.set_string(btn_x, btn_y, yes_lbl, yes_style);
        buf.set_string(btn_x + yes_w + gap, btn_y, no_lbl, no_style);

        Self::draw_shadow(buf, canvas, w, h);
    }

    // ── Helpers ──

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
        let styles = ratatui_cheese::input::InputStyles { text: Style::default().fg(palette::BG_1), ..Default::default() };
        let inp = ratatui_cheese::input::Input::new("").prompt("").placeholder("search profiles...").styles(styles);
        ratatui::widgets::StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }

    fn draw_scrollbar(buf: &mut Buffer, area: Rect, offset: usize, total: usize, view_h: usize, focused: bool) {
        if view_h == 0 {
            return;
        }
        if total <= view_h {
            for i in 0..view_h {
                let sx = area.right().saturating_sub(1);
                let sy = area.y + i as u16;
                buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
            }
            return;
        }
        let bar_h = ((view_h as f64 / total as f64) * view_h as f64).max(2.0) as usize;
        let bar_h = bar_h.min(view_h);
        let bar_y = ((offset as f64 / (total - view_h).max(1) as f64) * (view_h - bar_h) as f64) as usize;
        let thumb_fg = if focused { palette::PROCESSING_HEAT } else { palette::PROCESSING_BASE };
        for i in 0..view_h {
            let sx = area.right().saturating_sub(1);
            let sy = area.y + i as u16;
            if i >= bar_y && i < bar_y + bar_h {
                buf.set_string(sx, sy, "█", Style::default().fg(thumb_fg));
            } else {
                buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
            }
        }
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
