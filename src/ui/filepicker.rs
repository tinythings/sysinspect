use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, StatefulWidget, Widget},
};
use ratatui_cheese::input::{Input, InputState};
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::dashed_title;
use std::{
    cell::Cell,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerMode {
    DirectoryPicker,
    FilePicker,
    Any,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PickerFocus {
    Dirs,
    Files,
}

#[derive(Debug)]
struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_parent: bool,
    icon: &'static str,
}

#[derive(Debug)]
pub struct FilePicker {
    pub visible: bool,
    pub mode: PickerMode,
    pub current_path: PathBuf,
    pub selected: Option<PathBuf>,
    entries: Vec<DirEntry>,
    dirs_end: usize,
    dir_cursor: usize,
    file_cursor: usize,
    dir_scroll: Cell<usize>,
    file_scroll: Cell<usize>,
    focus: PickerFocus,
    filter_input: InputState,
    filter_focus: bool,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self {
            visible: false,
            mode: PickerMode::FilePicker,
            current_path: PathBuf::from("."),
            selected: None,
            entries: Vec::new(),
            dirs_end: 0,
            dir_cursor: 0,
            file_cursor: 0,
            dir_scroll: Cell::new(0),
            file_scroll: Cell::new(0),
            focus: PickerFocus::Dirs,
            filter_input: InputState::new(),
            filter_focus: false,
        }
    }
}

impl FilePicker {
    pub fn open(&mut self, path: &std::path::Path, mode: PickerMode) {
        self.visible = true;
        self.mode = mode;
        self.current_path = path.to_path_buf();
        self.selected = None;
        self.dir_cursor = 0;
        self.file_cursor = 0;
        self.dir_scroll = Cell::new(0);
        self.file_scroll = Cell::new(0);
        self.focus = PickerFocus::Dirs;
        self.filter_input = InputState::new();
        self.filter_focus = false;
        self.refresh_entries();
    }

    fn refresh_entries(&mut self) {
        self.entries.clear();
        self.dirs_end = 0;

        // Parent entry
        if let Some(parent) = self.current_path.parent() {
            self.entries.push(DirEntry { name: "..".into(), path: parent.to_path_buf(), is_dir: true, is_parent: true, icon: "↑" });
        }

        let filter = self.filter_input.value().to_lowercase();

        if let Ok(rd) = fs::read_dir(&self.current_path) {
            let mut dirs: Vec<DirEntry> = Vec::new();
            let mut files: Vec<DirEntry> = Vec::new();

            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                let is_dir = path.is_dir();

                if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                    continue;
                }

                let icon = Self::file_icon(&path, is_dir);

                let de = DirEntry { name, path, is_dir, is_parent: false, icon };

                if is_dir {
                    dirs.push(de);
                } else {
                    files.push(de);
                }
            }

            dirs.sort_by_key(|a| a.name.to_lowercase());
            files.sort_by_key(|a| a.name.to_lowercase());

            self.entries.append(&mut dirs);
            self.dirs_end = self.entries.len();
            self.entries.append(&mut files);
        }

        // Clamp cursors
        let n_dirs = self.dirs_end.max(1) - 1; // minus parent
        let n_files = self.entries.len().saturating_sub(self.dirs_end);
        self.dir_cursor = self.dir_cursor.min(n_dirs);
        self.file_cursor = self.file_cursor.min(n_files.saturating_sub(1));
    }

    fn meta_info_or_unknown(path: &std::path::Path) -> (String, String, String, String) {
        match fs::metadata(path) {
            Ok(m) => {
                let pm = m.permissions().mode();
                let mode = unix_mode_to_string(pm);
                let uid = m.uid();
                let gid = m.gid();

                // Convert numeric uid/gid to names (best effort)
                let user = unsafe { get_owner_name(uid) };
                let group = unsafe { get_group_name(gid) };

                let mtime = format_mtime(m.modified().ok());
                (mode, user, group, mtime)
            }
            Err(_) => ("??????????".into(), "???".into(), "???".into(), "??? ?? ??:??".into()),
        }
    }

    fn file_icon(path: &std::path::Path, is_dir: bool) -> &'static str {
        if is_dir {
            return "\u{1F4C1}"; // 📁
        }
        if let Ok(m) = fs::metadata(path)
            && m.permissions().mode() & 0o111 != 0
        {
            return "\u{1F4A5}"; // 💥 executable
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "txt" | "log" | "md" | "rs" | "py" | "sh" | "toml" | "yaml" | "yml" | "conf" | "json" | "ini" | "cfg" | "csv" => "\u{1F4C4}",
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" => "\u{1F5BC}",
            "iso" | "img" | "dmg" => "\u{1F4BF}",
            "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => "\u{1F4E6}",
            _ => "\u{1F4DC}", // 📜 default
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.visible {
            return false;
        }

        if self.filter_focus {
            match key.code {
                KeyCode::Esc => {
                    self.filter_focus = false;
                    self.focus = PickerFocus::Dirs;
                    self.refresh_entries();
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.filter_focus = false;
                    self.refresh_entries();
                }
                KeyCode::Enter => {
                    self.filter_focus = false;
                    self.refresh_entries();
                }
                KeyCode::Backspace => {
                    self.filter_input.delete_before();
                    self.refresh_entries();
                }
                KeyCode::Delete => {
                    self.filter_input.delete_at();
                    self.refresh_entries();
                }
                KeyCode::Left => {
                    self.filter_input.move_left();
                }
                KeyCode::Right => {
                    self.filter_input.move_right();
                }
                KeyCode::Home => {
                    self.filter_input.home();
                }
                KeyCode::End => {
                    self.filter_input.end();
                }
                KeyCode::Char(c) => {
                    self.filter_input.insert_char(c);
                    self.refresh_entries();
                }
                _ => {}
            }
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                self.visible = false;
            }
            KeyCode::Tab => {
                if self.mode == PickerMode::FilePicker && self.entries.len() > self.dirs_end {
                    self.focus = if self.focus == PickerFocus::Dirs { PickerFocus::Files } else { PickerFocus::Dirs };
                }
            }
            KeyCode::BackTab => {
                if self.mode == PickerMode::FilePicker {
                    self.focus = if self.focus == PickerFocus::Files { PickerFocus::Dirs } else { PickerFocus::Files };
                }
            }
            KeyCode::Up => match self.focus {
                PickerFocus::Dirs => {
                    self.dir_cursor = self.dir_cursor.saturating_sub(1);
                }
                PickerFocus::Files => {
                    self.file_cursor = self.file_cursor.saturating_sub(1);
                }
            },
            KeyCode::Down => match self.focus {
                PickerFocus::Dirs => {
                    let max = self.dirs_end.saturating_sub(1);
                    self.dir_cursor = (self.dir_cursor + 1).min(max);
                }
                PickerFocus::Files => {
                    let max = self.entries.len().saturating_sub(self.dirs_end).saturating_sub(1);
                    self.file_cursor = (self.file_cursor + 1).min(max);
                }
            },
            KeyCode::PageUp => {
                let page = 10usize;
                match self.focus {
                    PickerFocus::Dirs => {
                        self.dir_cursor = self.dir_cursor.saturating_sub(page);
                    }
                    PickerFocus::Files => {
                        self.file_cursor = self.file_cursor.saturating_sub(page);
                    }
                }
            }
            KeyCode::PageDown => {
                let page = 10usize;
                match self.focus {
                    PickerFocus::Dirs => {
                        let max = self.dirs_end.saturating_sub(1);
                        self.dir_cursor = (self.dir_cursor + page).min(max);
                    }
                    PickerFocus::Files => {
                        let max = self.entries.len().saturating_sub(self.dirs_end).saturating_sub(1);
                        self.file_cursor = (self.file_cursor + page).min(max);
                    }
                }
            }
            KeyCode::Enter => {
                let idx = match self.focus {
                    PickerFocus::Dirs => self.dir_cursor,
                    PickerFocus::Files => self.dirs_end + self.file_cursor,
                };

                if let Some(entry) = self.entries.get(idx)
                    && (entry.is_parent || entry.is_dir)
                {
                    self.current_path = entry.path.clone();
                    self.filter_input = InputState::new();
                    self.dir_cursor = 0;
                    self.file_cursor = 0;
                    self.refresh_entries();
                }
            }
            KeyCode::Char(' ') => {
                if self.mode == PickerMode::DirectoryPicker {
                    self.selected = Some(self.current_path.clone());
                    self.visible = false;
                } else {
                    let idx = match self.focus {
                        PickerFocus::Dirs => self.dir_cursor,
                        PickerFocus::Files => self.dirs_end + self.file_cursor,
                    };
                    if let Some(entry) = self.entries.get(idx) {
                        let selectable = if self.mode == PickerMode::Any { !entry.is_parent } else { !entry.is_parent && !entry.is_dir };
                        if selectable {
                            self.selected = Some(entry.path.clone());
                            self.filter_input = InputState::new();
                            self.visible = false;
                        }
                    }
                }
            }
            KeyCode::Char('/') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter_focus = true;
                self.filter_input = InputState::new();
            }
            _ => {}
        }

        true
    }

    pub fn status_line(&self) -> Line<'static> {
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(palette::FG)),
            Span::styled("to navigate,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Space ", Style::default().fg(palette::FG)),
            Span::styled("to select,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("to cancel", Style::default().fg(palette::FAINT)),
        ])
    }

    pub fn render(&self, parent: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }

        let dlg_w = (parent.width * 3 / 4).clamp(60, 80);
        let dlg_h = (parent.height * 3 / 4).clamp(14, 24);
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

        let title_text = match self.mode {
            PickerMode::DirectoryPicker => " Directory Selector ",
            PickerMode::FilePicker => " File Selector ",
            PickerMode::Any => " Module Selector ",
        };
        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        title::overlay_gradient_title(
            buf,
            canvas,
            &title_style,
            &[TitleSegment { text: title_text.into(), bg: palette::PROCESSING_BASE, fg: palette::FG }],
        );

        if inner.height < 4 {
            return;
        }

        let mut row_y = inner.y;

        // ── Filter row ──
        let filter_label = if self.filter_focus { " / \u{2192} " } else { " / " };
        let fl_style =
            if self.filter_focus { Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD) } else { Style::default().fg(palette::MUTED) };
        buf.set_string(inner.x + 1, row_y, filter_label, fl_style);

        let input_x = inner.x + 5;
        let input_w = inner.width.saturating_sub(7);
        if input_w > 0 {
            let mut is = Self::copy_input_state(&self.filter_input, self.filter_focus);
            let inp = Input::new("").prompt("").placeholder("filter...");
            StatefulWidget::render(&inp, Rect::new(input_x, row_y, input_w, 1), buf, &mut is);
        }

        let filter_active = !self.filter_input.value().is_empty() || self.filter_focus;
        let filter_line = filter_active as u16;
        row_y += 1;

        let sections: u16 = if self.mode == PickerMode::FilePicker { 2 } else { 1 };
        let available = inner.height.saturating_sub(1).saturating_sub(row_y.saturating_sub(inner.y)).saturating_sub(filter_line);
        let dir_rows = if sections == 2 { available / 2 } else { available };

        // ── Directories section ──
        dashed_title(
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
            buf,
            " Directories ",
            palette::PROCESSING,
            palette::PROCESSING,
            palette::PROCESSING_DIMMED,
        );
        row_y += 1;

        let dir_list = self.entries.iter().take(self.dirs_end).collect::<Vec<_>>();
        let dir_end = (row_y + dir_rows).min(inner.y + inner.height);
        let dir_area = Rect { x: inner.x + 1, y: row_y, width: inner.width.saturating_sub(1), height: dir_rows.min(dir_end.saturating_sub(row_y)) };

        self.render_section(dir_area, buf, &dir_list, self.dir_cursor, self.focus == PickerFocus::Dirs, &self.dir_scroll);
        row_y = dir_area.y + dir_area.height;

        // ── Files section (FilePicker only) ──
        if self.mode == PickerMode::FilePicker && row_y + 1 < inner.y + inner.height {
            dashed_title(
                Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
                buf,
                " Files ",
                palette::PROCESSING,
                palette::PROCESSING,
                palette::PROCESSING_DIMMED,
            );
            row_y += 1;

            let file_list = self.entries.iter().skip(self.dirs_end).collect::<Vec<_>>();
            let file_end = (row_y + (available - dir_rows).saturating_sub(1)).min(inner.y + inner.height);
            let file_area = Rect {
                x: inner.x + 1,
                y: row_y,
                width: inner.width.saturating_sub(1),
                height: (available - dir_rows).saturating_sub(1).min(file_end.saturating_sub(row_y)),
            };

            self.render_section(file_area, buf, &file_list, self.file_cursor, self.focus == PickerFocus::Files, &self.file_scroll);
        }

        // MS-DOS shadow
        let buf_area = buf.area();
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

    fn render_section(&self, area: Rect, buf: &mut Buffer, entries: &[&DirEntry], cursor: usize, active: bool, scroll: &Cell<usize>) {
        if area.height == 0 || entries.is_empty() {
            return;
        }

        let mut s = scroll.get();
        let view_h = area.height as usize;
        let total = entries.len();
        let max_scroll = total.saturating_sub(view_h);

        if cursor < s {
            s = cursor;
        }
        if cursor >= s + view_h {
            s = cursor.saturating_sub(view_h.saturating_sub(1));
        }
        s = s.min(max_scroll);
        scroll.set(s);

        let visible = entries.iter().skip(s).take(view_h);

        let muted = Style::default().fg(palette::MUTED);
        let hl_style = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);
        let muted_hl = Style::default().fg(palette::BG_0).bg(palette::HIGHLIGHT);

        // Calculate field widths for alignment (includes icon + spaces)
        let longest_name = entries
            .iter()
            .map(|e| {
                let icon = if e.is_parent { "↑ " } else { e.icon };
                UnicodeWidthStr::width(format!("    {icon} {}", e.name).as_str())
            })
            .max()
            .unwrap_or(10);

        for (i, entry) in visible.enumerate() {
            let abs_idx = scroll.get() + i;
            let ry = area.y + i as u16;
            if ry >= area.y + area.height {
                break;
            }

            let is_selected = abs_idx == cursor && active;
            let row_style = if is_selected { hl_style } else { Style::default().fg(palette::FG) };

            let icon = if entry.is_parent { "↑ " } else { entry.icon };
            let line = format!("    {icon} {}", entry.name);

            if !entry.is_parent {
                let (mode, user, group, mtime) = Self::meta_info_or_unknown(&entry.path);
                let info = format!(" {}  {}  {}  {}", mode, user, group, mtime);
                let info_x = area.x + 3 + longest_name as u16;
                let name_end = info_x.saturating_sub(1); // leave gap before info
                let max_name_w = name_end.saturating_sub(area.x + 1);
                let name_trimmed = truncate_to_width(&line, max_name_w);
                buf.set_string(area.x + 1, ry, &name_trimmed, row_style);
                if info_x < area.right() {
                    let info_style = if is_selected { muted_hl } else { muted };
                    buf.set_string(info_x, ry, &info, info_style);
                }
            } else {
                buf.set_string(area.x + 1, ry, &line, row_style);
            }

            // Re-paint with highlight if selected
            if is_selected {
                if !entry.is_parent {
                    let name_end = (area.x + 3 + longest_name as u16).saturating_sub(1);
                    let max_name_w = name_end.saturating_sub(area.x + 1);
                    let name_trimmed = truncate_to_width(&line, max_name_w);
                    buf.set_string(area.x + 1, ry, &name_trimmed, row_style);
                } else {
                    buf.set_string(area.x + 1, ry, &line, row_style);
                }
            }
        }

        // Scrollbar
        if total > view_h {
            let bar_h = ((view_h as f64 / total as f64) * view_h as f64).max(1.0) as usize;
            let bar_y = ((s as f64 / total as f64) * (view_h - bar_h) as f64) as usize;
            for i in 0..view_h {
                let sx = area.right().saturating_sub(1);
                let sy = area.y + i as u16;
                if i >= bar_y && i < bar_y + bar_h {
                    buf.set_string(sx, sy, "█", Style::default().fg(palette::PROCESSING_HEAT));
                } else {
                    buf.set_string(sx, sy, "│", Style::default().fg(palette::MUTED));
                }
            }
        }
    }

    fn copy_input_state(src: &InputState, focused: bool) -> InputState {
        let mut is = InputState::new();
        is.set_value(src.value().to_string());
        is.set_focused(focused);
        let fc = src.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        is
    }
}

fn unix_mode_to_string(mode: u32) -> String {
    let mut s = String::with_capacity(10);
    s.push(if mode & 0o040000 != 0 { 'd' } else { '-' });
    s.push(if mode & 0o00400 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o00200 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o00100 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o00040 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o00020 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o00010 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o00004 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o00002 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o00001 != 0 { 'x' } else { '-' });
    s
}

unsafe fn get_owner_name(_uid: u32) -> String {
    "user".to_string()
}

unsafe fn get_group_name(_gid: u32) -> String {
    "group".to_string()
}

fn format_mtime(modified: Option<std::time::SystemTime>) -> String {
    match modified {
        Some(t) => {
            use chrono::{DateTime, Local};
            let dt: DateTime<Local> = t.into();
            dt.format("%b %d %H:%M").to_string()
        }
        None => "??? ?? ??:??".to_string(),
    }
}

fn truncate_to_width(s: &str, max_w: u16) -> String {
    let mut w: u16 = 0;
    s.chars()
        .take_while(|c| {
            w += unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0) as u16;
            w <= max_w
        })
        .collect()
}
