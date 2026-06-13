use super::{
    dslbrowser, palette, platforms, profiles,
    title::{self, TitleSegment, TitleStyle},
};
use indexmap::IndexMap;
use libsysinspect::console::{ConsoleModuleArgument, ConsoleModuleRow};
use ratatui::{
    layout::{Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, StatefulWidget, Tabs, Widget},
};
use ratatui_cheese::input::{Input, InputState, InputStyles};
use ratatui_glamour::color::{blend_2d, lerp_color};
use ratatui_glamour::rule::dashed_title;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct StagedModule {
    pub name: String,
    pub version: Option<String>,
    pub descr: String,
    pub path: std::path::PathBuf,
    pub checked: bool,
    pub platform: Option<String>,
    pub arch: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagingFocus {
    List,
    AddSelected,
    Cancel,
    CrossPlatformDelete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagingMode {
    ModuleAdd,
    ModuleDelete,
    ProfileModuleAdd,
    ProfileLibraryAdd,
}

#[derive(Debug)]
pub struct RepoManager {
    pub visible: bool,
    pub module_groups: IndexMap<String, Vec<ConsoleModuleRow>>,
    pub group_order: Vec<String>,
    pub group_cursor: usize,
    pub group_cursor_row: usize,
    pub group_expanded: Vec<bool>,
    pub group_scrolls: IndexMap<String, Cell<usize>>,

    // Staging
    pub staging: bool,
    pub staged: Vec<StagedModule>,
    pub staging_cursor: usize,
    pub staging_scroll: Cell<usize>,
    pub staging_focus: StagingFocus,
    pub staging_mode: StagingMode,
    pub delete_mode: bool,
    pub cross_platform_delete: bool,

    // Progress
    pub progress: Arc<Mutex<Option<(usize, usize)>>>,

    // Signals
    pub bulk_add_triggered: bool,
    pub bulk_delete_triggered: bool,
    pub needs_reload: bool,

    // Filter
    pub filter: InputState,
    pub filter_focus: bool,

    // Module info popup
    pub info_visible: bool,
    pub info_row: usize,
    pub info_tab: u8,
    pub info_scroll: Cell<usize>,
    pub info_active_tab: u8,

    // Tabs
    pub active_tab: u8,
    pub lib_rows: Vec<libsysinspect::console::ConsoleLibraryRow>,
    pub lib_cursor: usize,
    pub lib_scroll: Cell<usize>,

    // Profiles
    pub profiles: profiles::ProfilesManager,

    // Platforms
    pub platforms: platforms::PlatformsManager,
}

impl Default for RepoManager {
    fn default() -> Self {
        Self {
            visible: false,
            module_groups: IndexMap::new(),
            group_order: Vec::new(),
            group_cursor: 0,
            group_cursor_row: 0,
            group_expanded: Vec::new(),
            group_scrolls: IndexMap::new(),
            staging: false,
            staged: Vec::new(),
            staging_cursor: 0,
            staging_scroll: Cell::new(0),
            staging_focus: StagingFocus::List,
            staging_mode: StagingMode::ModuleAdd,
            delete_mode: false,
            cross_platform_delete: false,
            progress: Arc::new(Mutex::new(None)),
            bulk_add_triggered: false,
            bulk_delete_triggered: false,
            needs_reload: false,
            filter: InputState::new(),
            filter_focus: false,
            info_visible: false,
            info_row: 0,
            info_tab: 0,
            info_scroll: Cell::new(0),
            info_active_tab: 0,
            active_tab: 0,
            lib_rows: Vec::new(),
            lib_cursor: 0,
            lib_scroll: Cell::new(0),
            profiles: profiles::ProfilesManager::default(),
            platforms: platforms::PlatformsManager::default(),
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
        self.delete_mode = false;
        self.cross_platform_delete = false;
        self.staged.clear();
    }

    pub fn focused_module(&self) -> Option<&ConsoleModuleRow> {
        if self.group_cursor_row == 0 { return None; }
        let key = self.group_order.get(self.group_cursor)?;
        self.module_groups.get(key)?.get(self.group_cursor_row - 1)
    }

    pub fn focused_group_modules(&self) -> Option<&Vec<ConsoleModuleRow>> {
        let key = self.group_order.get(self.group_cursor)?;
        self.module_groups.get(key)
    }

    pub fn focused_group_name(&self) -> Option<&str> {
        self.group_order.get(self.group_cursor).map(|s| s.as_str())
    }

    pub fn filtered_module_count(&self, filter_value: &str) -> usize {
        let f = filter_value.to_lowercase();
        self.module_groups
            .values()
            .flat_map(|g| g.iter())
            .filter(|r| f.is_empty() || r.name.to_lowercase().contains(&f) || r.descr.to_lowercase().contains(&f))
            .count()
    }

    pub fn focused_module_for_info(&self) -> Option<&ConsoleModuleRow> {
        let key = self.group_order.get(self.group_cursor)?;
        // info_row is 1-indexed (0 = header)
        if self.info_row == 0 { return None; }
        self.module_groups.get(key)?.get(self.info_row - 1)
    }

    pub fn enter_profile_module_staging(&mut self) {
        self.staged = self
            .module_groups
            .values()
            .flat_map(|g| g.iter())
            .map(|r| StagedModule {
                name: r.name.clone(),
                version: r.version.clone(),
                descr: r.descr.clone(),
                path: std::path::PathBuf::new(),
                checked: false,
                platform: Some(r.platform.clone()),
                arch: Some(r.arch.clone()),
            })
            .collect();
        self.staging_cursor = 0;
        self.staging_scroll = Cell::new(0);
        self.staging_focus = StagingFocus::List;
        self.staging_mode = StagingMode::ProfileModuleAdd;
        self.profiles.detail_visible = false;
        self.staging = true;
        self.delete_mode = false;
    }

    pub fn enter_profile_library_staging(&mut self) {
        self.staged = self
            .lib_rows
            .iter()
            .map(|r| StagedModule {
                name: r.name.clone(),
                version: Some(r.kind.clone()),
                descr: r.checksum.clone(),
                path: std::path::PathBuf::new(),
                checked: false,
                platform: None,
                arch: None,
            })
            .collect();
        self.staging_cursor = 0;
        self.staging_scroll = Cell::new(0);
        self.staging_focus = StagingFocus::List;
        self.staging_mode = StagingMode::ProfileLibraryAdd;
        self.profiles.detail_visible = false;
        self.staging = true;
        self.delete_mode = false;
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
                    AddSelected => if self.delete_mode { CrossPlatformDelete } else { Cancel },
                    CrossPlatformDelete => Cancel,
                    Cancel => List,
                };
            }
            crossterm::event::KeyCode::BackTab => {
                use StagingFocus::*;
                self.staging_focus = match self.staging_focus {
                    List => Cancel,
                    AddSelected => List,
                    Cancel => if self.delete_mode { CrossPlatformDelete } else { AddSelected },
                    CrossPlatformDelete => AddSelected,
                };
            }
            crossterm::event::KeyCode::Char(' ') if self.staging_focus == StagingFocus::CrossPlatformDelete => {
                self.cross_platform_delete = !self.cross_platform_delete;
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
                    if self.delete_mode {
                        self.bulk_delete_triggered = true;
                    } else {
                        self.bulk_add_triggered = true;
                    }
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
        self.render_main(parent, buf);
        if self.progress.lock().unwrap().is_some() {
            self.render_progress(parent, buf);
        }
        if self.info_visible {
            self.render_info(parent, buf);
        }
        if self.staging {
            self.render_staging(parent, buf);
        }
        if self.profiles.detail_visible {
            self.profiles.render_detail(parent, buf);
        }
        if self.profiles.create_visible {
            self.profiles.render_create(parent, buf);
        }
        if self.profiles.delete_visible {
            self.profiles.render_delete(parent, buf);
        }
        if self.platforms.delete_visible {
            self.platforms.render_delete(parent, buf);
        }
    }

    fn render_main(&self, parent: Rect, buf: &mut Buffer) {
        let dlg_w = (parent.width * 3 / 4).clamp(70, 110);
        let dlg_h = (parent.height * 3 / 4).clamp(10, 24);
        let x = parent.x + (parent.width.saturating_sub(dlg_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(dlg_h)) / 2;
        let canvas = Rect { x, y, width: dlg_w, height: dlg_h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_3, palette::BG_1] as &[Color]);
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

        let tab_names = ["Modules", "Libraries", "Profiles", "Platforms"];
        let section_name = tab_names[self.active_tab as usize];
        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        title::overlay_gradient_title(
            buf,
            canvas,
            &title_style,
            &[
                TitleSegment { text: " Artefacts ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() },
                TitleSegment { text: format!(" {section_name} "), bg: palette::PROCESSING_HEAT, fg: palette::FG, modifier: Modifier::empty() },
            ],
        );

        if inner.height < 4 {
            return;
        }
        let [tabs_area, body] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();
        let titles: Vec<Line> = tab_names.iter().map(|t| Line::from(format!(" {t} "))).collect();
        Tabs::new(titles)
            .select(self.active_tab as usize)
            .divider("\u{E0B1}")
            .style(Style::default().fg(palette::MUTED))
            .highlight_style(Style::default().fg(palette::GRAY_2).add_modifier(Modifier::BOLD))
            .render(tabs_area, buf);

        match self.active_tab {
            0 => self.render_modules(body, buf),
            1 => self.render_libraries(body, buf),
            2 => self.profiles.render_list(body, buf, self.filter_focus, &self.filter),
            3 => self.platforms.render_list(body, buf, self.filter_focus, &self.filter),
            _ => {}
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

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_3, palette::BG_1] as &[Color]);
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
            &[TitleSegment {
                text: " Module and Library Manager ".into(),
                bg: palette::PROCESSING_BASE,
                fg: palette::FG,
                modifier: Modifier::empty(),
            }],
        );

        if inner.height < 6 || self.staged.is_empty() {
            return;
        }

        let list_height = inner.height.saturating_sub(btn_height).saturating_sub(if self.delete_mode { 1 } else { 0 });

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

        // Cross-platform delete checkbox
        if self.delete_mode {
            let chk_y = inner.y + list_height + 1;
            let (chk, chk_style) = if self.cross_platform_delete {
                ("▣", Style::default().fg(palette::SUCCESS))
            } else {
                ("□", Style::default().fg(palette::GRAY_1))
            };
            let chk_text = " Delete across all platforms";
            let sel = self.staging_focus == StagingFocus::CrossPlatformDelete;
            if sel {
                for cx in 0..inner.width {
                    if let Some(cell) = buf.cell_mut(Position::new(inner.x + cx, chk_y)) {
                        cell.set_bg(palette::HIGHLIGHT);
                    }
                }
            }
            let row_style = if sel { Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT) } else { Style::default().fg(palette::FG) };
            buf.set_string(inner.x + 1, chk_y, chk, if sel { row_style } else { chk_style });
            buf.set_string(inner.x + 5, chk_y, chk_text, row_style);
        }

        // Buttons
        let btn_y = inner.y + list_height + (if self.delete_mode { 2 } else { 1 });
        let action_label = if self.delete_mode { "[ Delete ]" } else { "[ Add Selected ]" };
        let cancel_label = "[ Cancel ]";
        let action_w = action_label.len() as u16;
        let cancel_w = cancel_label.len() as u16;
        let total_btn_w = action_w + cancel_w + 6;
        let btn_x = inner.x + (inner.width.saturating_sub(total_btn_w)) / 2;

        let sel_btn = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let unsel_btn = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let action_style = if self.staging_focus == StagingFocus::AddSelected { sel_btn } else { unsel_btn };
        let cancel_style = if self.staging_focus == StagingFocus::Cancel { sel_btn } else { unsel_btn };

        buf.set_string(btn_x, btn_y, action_label, action_style);
        buf.set_string(btn_x + action_w + 4, btn_y, cancel_label, cancel_style);

        Self::draw_shadow(buf, canvas, dlg_w, dlg_h);
    }

    fn render_progress(&self, parent: Rect, buf: &mut Buffer) {
        let (done, total) = match *self.progress.lock().unwrap() {
            Some(p) => p,
            None => return,
        };

        let dlg_w = (parent.width / 2).clamp(50, 80);
        let dlg_h = 6u16;
        let x = parent.x + (parent.width.saturating_sub(dlg_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(dlg_h)) / 2;
        let canvas = Rect { x, y, width: dlg_w, height: dlg_h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 13.0, &[palette::GRAY_0, palette::PROCESSING_GLOW] as &[Color]);
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

        let bar_y = inner.y + 1;
        let bar_w = inner.width.saturating_sub(2);
        let filled = (bar_w as usize * done).checked_div(total).map(|v| v as u16).unwrap_or(0);

        // Draw filled and unfilled portions
        if filled > 0 {
            buf.set_string(inner.x + 1, bar_y, "█".repeat(filled as usize), Style::default().fg(palette::PROCESSING_PEAK));
        }
        if filled < bar_w {
            let unfilled = (bar_w - filled) as usize;
            buf.set_string(inner.x + 1 + filled, bar_y, "─".repeat(unfilled), Style::default().fg(palette::MUTED));
        }

        // Percentage
        let pct = (done * 100).checked_div(total).map(|p| format!("{p}%")).unwrap_or_else(|| "0%".into());
        let pct_x = inner.x + (inner.width.saturating_sub(pct.len() as u16)) / 2;
        buf.set_string(pct_x, bar_y, &pct, Style::default().fg(palette::FG).add_modifier(Modifier::BOLD));

        // Cancel button
        let cancel = "[ Cancel ]";
        let btn_x = inner.x + (inner.width.saturating_sub(cancel.len() as u16)) / 2;
        buf.set_string(btn_x, bar_y + 1, cancel, Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD));

        Self::draw_shadow(buf, canvas, dlg_w, dlg_h);
    }

    fn render_modules(&self, inner: Rect, buf: &mut Buffer) {
        if inner.height < 2 {
            return;
        }
        let [filter_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();
        Self::render_filter_row(filter_area, buf, self.filter_focus, &self.filter);

        if self.module_groups.is_empty() {
            let msg = "(no modules found)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }

        let flt = self.filter.value().to_lowercase();
        let name_w: u16 = 28;
        let ver_w: u16 = 6;
        let mut row_y = list_area.y;
        let preview_rows: usize = 3;

        for (gi, key) in self.group_order.iter().enumerate() {
            if row_y >= list_area.bottom() { break; }
            let modules = match self.module_groups.get(key) { Some(m) => m, None => continue };
            let filtered: Vec<&ConsoleModuleRow> = modules.iter()
                .filter(|r| flt.is_empty() || r.name.to_lowercase().contains(&flt) || r.descr.to_lowercase().contains(&flt))
                .collect();
            let count = filtered.len();
            let expanded = self.group_expanded.get(gi).copied().unwrap_or(false);
            let chevron = if expanded { "▼" } else { "▶" };
            let focused = !self.filter_focus && gi == self.group_cursor;
            let header_fg = if focused { palette::HIGHLIGHT } else { palette::MUTED };
            let count_text = format!(" ({count})");

            // Header row
            buf.set_string(list_area.x + 1, row_y, chevron, Style::default().fg(header_fg).add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }));
            let label = format!(" {key}{count_text} ");
            buf.set_string(list_area.x + 4, row_y, &label, Style::default().fg(header_fg).add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() }));
            let label_w = UnicodeWidthStr::width(label.as_str()) as u16;
            let fill_start = list_area.x + 4 + label_w;
            let fill_end = list_area.right().saturating_sub(1);
            for fx in fill_start..fill_end {
                if let Some(cell) = buf.cell_mut(Position::new(fx, row_y)) {
                    let t = if fill_end > fill_start + 1 {
                        (fx - fill_start) as f32 / (fill_end - fill_start).saturating_sub(1) as f32
                    } else { 0.0 };
                    let color = lerp_color(palette::PRIMARY, palette::PROCESSING_DIMMED, t);
                    cell.set_char('/');
                    cell.set_fg(color);
                }
            }
            row_y += 1;
            if row_y >= list_area.bottom() { break; }

            if expanded {
                // Expanded: show all rows with scrollbar
                let remaining = (list_area.bottom().saturating_sub(row_y)) as usize;
                if remaining == 0 { continue; }
                let view_h = remaining.min(count);
                let total = filtered.len();
                let max_scroll = total.saturating_sub(view_h);
                let mut s = self.group_scrolls.get(key).map(|c| c.get()).unwrap_or(0).min(max_scroll);
                let cursor_in_group = if focused && gi == self.group_cursor && self.group_cursor_row > 0 {
                    Some(self.group_cursor_row - 1)  // 1-indexed → 0-indexed
                } else { None };
                if let Some(c) = cursor_in_group {
                    if c < s { s = c; }
                    if c >= s + view_h { s = c.saturating_sub(view_h.saturating_sub(1)); }
                    s = s.min(max_scroll);
                }
                if let Some(cell) = self.group_scrolls.get(key) { cell.set(s); }
                self.render_module_rows(list_area, &filtered, s, view_h, row_y, focused, cursor_in_group, name_w, ver_w, buf);
                if total > view_h {
                    self.draw_scrollbar(buf, Rect { x: list_area.x, y: row_y, width: list_area.width, height: view_h as u16 }, s, total, view_h);
                }
                row_y += view_h as u16;
            } else if count > 0 {
                // Collapsed: show preview rows + summary
                let show = preview_rows.min(count);
                let cursor_in_group = if focused && gi == self.group_cursor && self.group_cursor_row > 0 && self.group_cursor_row <= preview_rows {
                    Some(self.group_cursor_row - 1)
                } else { None };
                for i in 0..show {
                    if row_y >= list_area.bottom() { break; }
                    if let Some(row) = filtered.get(i) {
                        render_module_row(list_area, row_y, row, focused && cursor_in_group == Some(i), name_w, ver_w, buf);
                    }
                    row_y += 1;
                }
                if count > preview_rows && row_y < list_area.bottom() {
                    let more = format!("  ({more})...", more = count - preview_rows);
                    buf.set_string(list_area.x + 1, row_y, &more, Style::default().fg(palette::MUTED));
                    row_y += 1;
                }
            }
            // Empty group with 0 modules: just the header, no rows
        }
    }

    fn render_module_rows(&self, area: Rect, filtered: &[&ConsoleModuleRow], offset: usize, view_h: usize, start_y: u16, focused: bool, cursor: Option<usize>, name_w: u16, ver_w: u16, buf: &mut Buffer) {
        for i in 0..view_h {
            let idx = offset + i;
            let ry = start_y + i as u16;
            if let Some(row) = filtered.get(idx) {
                render_module_row(area, ry, row, focused && cursor == Some(idx), name_w, ver_w, buf);
            }
        }
    }

    fn render_libraries(&self, inner: Rect, buf: &mut Buffer) {
        if inner.height < 2 {
            return;
        }
        let [filter_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();
        Self::render_filter_row(filter_area, buf, self.filter_focus, &self.filter);
        if self.lib_rows.is_empty() {
            let msg = "(no libraries found)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }
        let flt = self.filter.value().to_lowercase();
        let filtered: Vec<(usize, &libsysinspect::console::ConsoleLibraryRow)> = self
            .lib_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| flt.is_empty() || r.name.to_lowercase().contains(&flt) || r.kind.to_lowercase().contains(&flt))
            .collect();
        let view_h = list_area.height as usize;
        let total = filtered.len();
        let max_scroll = total.saturating_sub(view_h);
        let mut s = self.lib_scroll.get();
        let cursor = self.lib_cursor.min(total.saturating_sub(1));
        if cursor < s {
            s = cursor;
        }
        if cursor >= s + view_h {
            s = cursor.saturating_sub(view_h.saturating_sub(1));
        }
        s = s.min(max_scroll);
        self.lib_scroll.set(s);
        if total == 0 {
            let msg = "(no matches)";
            let x = list_area.x + (list_area.width.saturating_sub(msg.len() as u16)) / 2;
            let y = list_area.y + list_area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(palette::MUTED));
            return;
        }
        let hl = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);
        let kind_w: u16 = 8;
        let name_w = list_area.width.saturating_sub(kind_w + 38);
        let sum_w = 30u16;
        for i in 0..view_h.min(total.saturating_sub(s)) {
            let fi = s + i;
            let (_oi, row) = filtered[fi];
            let ry = list_area.y + i as u16;
            let sel = !self.filter_focus && fi == cursor;
            let row_style = if sel { hl } else { Style::default().fg(palette::FG) };
            if sel {
                for cx in 0..list_area.width {
                    if let Some(cell) = buf.cell_mut(Position::new(list_area.x + cx, ry)) {
                        cell.set_bg(palette::HIGHLIGHT);
                    }
                }
            }
            let kind_style = if sel { row_style } else { Style::default().fg(palette::PROCESSING) };
            buf.set_string(list_area.x + 1, ry, format!(" {}", truncate_str(&row.kind, kind_w as usize)), kind_style);
            let name_style = if sel { row_style } else { Style::default().fg(palette::FG) };
            buf.set_string(list_area.x + 1 + kind_w + 1, ry, truncate_str(&row.name, name_w as usize), name_style);
            let sum_style = if sel { row_style } else { Style::default().fg(palette::GRAY_1) };
            let sum_x = list_area.x + 1 + kind_w + 1 + name_w + 1;
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

        let input_x = area.x + 10u16;
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
        let inp = Input::new("").prompt("").placeholder("search name/description...").styles(styles);
        StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }

    pub fn handle_info_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        if !self.info_visible {
            return false;
        }
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.info_visible = false;
            }
            crossterm::event::KeyCode::Enter => {
                self.info_visible = false;
            }
            crossterm::event::KeyCode::Left if self.info_active_tab == 0 => {
                self.info_tab = self.info_tab.saturating_sub(1);
                self.info_scroll.set(0);
            }
            crossterm::event::KeyCode::Right if self.info_active_tab == 0 => {
                self.info_tab = (self.info_tab + 1).min(3);
                self.info_scroll.set(0);
            }
            crossterm::event::KeyCode::Up => {
                let s = self.info_scroll.get();
                self.info_scroll.set(s.saturating_sub(1));
            }
            crossterm::event::KeyCode::Down => {
                let s = self.info_scroll.get();
                self.info_scroll.set(s.saturating_add(1));
            }
            crossterm::event::KeyCode::PageUp => {
                let s = self.info_scroll.get();
                self.info_scroll.set(s.saturating_sub(10));
            }
            crossterm::event::KeyCode::PageDown => {
                let s = self.info_scroll.get();
                self.info_scroll.set(s.saturating_add(10));
            }
            _ => {}
        }
        true
    }

    fn render_info(&self, parent: Rect, buf: &mut Buffer) {
        match self.info_active_tab {
            0 => self.render_module_info(parent, buf),
            1 => self.render_library_info(parent, buf),
            _ => {}
        }
    }

    fn render_module_info(&self, parent: Rect, buf: &mut Buffer) {
        let row = match self.focused_module_for_info() {
            Some(r) => r,
            None => return,
        };
        let w = (parent.width * 80 / 100).max(60).min(parent.width.saturating_sub(2));
        let h = (parent.height * 80 / 100).clamp(12, 24);
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
            &[TitleSegment { text: format!(" {} ({} {}) ", row.name, row.platform, row.arch), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        if inner.height < 4 {
            return;
        }
        let [tabs_area, body_area, btn_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();

        let titles: Vec<Line> = ["Description", "Arguments", "Options", "Manual"].iter().map(|t| Line::from(format!(" {t} "))).collect();
        Tabs::new(titles)
            .select(self.info_tab as usize)
            .divider("\u{E0B1}")
            .style(Style::default().fg(palette::MUTED))
            .highlight_style(Style::default().fg(palette::GRAY_2).add_modifier(Modifier::BOLD))
            .render(tabs_area, buf);

        let section_title = ["Description", "Arguments", "Options", "Manual Page"][self.info_tab as usize];
        let mut yy = body_area.y;
        dashed_title(
            Rect { x: body_area.x, y: yy, width: body_area.width, height: 1 },
            buf,
            &format!(" {section_title} "),
            palette::PROCESSING,
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        yy += 1;
        yy += 1;

        let content_area = Rect { x: body_area.x, y: yy, width: body_area.width.saturating_sub(1), height: body_area.height.saturating_sub(2) };
        let muted = Style::default().fg(palette::MUTED);

        match self.info_tab {
            0 => {
                let desc = if row.descr.is_empty() { "Description is not available" } else { &row.descr };
                self.render_info_text(content_area, buf, desc);
            }
            1 => {
                if let Some(ref args) = row.args
                    && !args.is_empty()
                {
                    self.render_args_opts(content_area, buf, args, false);
                } else {
                    self.render_placeholder(content_area, buf, "This module has no arguments", &muted);
                }
            }
            2 => {
                if let Some(ref opts) = row.opts
                    && !opts.is_empty()
                {
                    self.render_args_opts(content_area, buf, opts, true);
                } else {
                    self.render_placeholder(content_area, buf, "This module has no options", &muted);
                }
            }
            3 => {
                if let Some(ref man) = row.manpage
                    && !man.is_empty()
                {
                    let rendered: Vec<ratatui::text::Line> = man.split('\n').map(|line| render_markup_spans(line)).collect();
                    self.render_info_lines(content_area, buf, &rendered);
                } else {
                    self.render_placeholder(content_area, buf, "Manual page is not available", &muted);
                }
            }
            _ => {}
        }

        // Close button
        let close_lbl = "[ Close ]";
        let close_w = close_lbl.len() as u16;
        let btn_x = btn_area.x + (btn_area.width.saturating_sub(close_w)) / 2;
        Paragraph::new(close_lbl)
            .style(Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD))
            .render(Rect::new(btn_x, btn_area.y, close_w, 1), buf);

        // Change default temporarily for shadow computation
        Self::draw_shadow(buf, canvas, w, h);
    }

    fn render_library_info(&self, parent: Rect, buf: &mut Buffer) {
        let lib = match self.lib_rows.get(self.info_row) {
            Some(r) => r,
            None => return,
        };
        let w = (parent.width * 60 / 100).max(50).min(parent.width.saturating_sub(2));
        let h: u16 = 8;
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
            &[TitleSegment { text: format!(" {} ", lib.name), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        let key_style = Style::default().fg(palette::PROCESSING).add_modifier(Modifier::BOLD);
        let val_style = Style::default().fg(palette::FG);

        let lines = [("Name:    ", &lib.name), ("Type:    ", &lib.kind), ("Sha256:  ", &lib.checksum)];
        for (i, (label, value)) in lines.iter().enumerate() {
            let ry = inner.y + 1 + i as u16;
            buf.set_string(inner.x + 3, ry, label, key_style);
            buf.set_string(
                inner.x + 3 + label.len() as u16,
                ry,
                truncate_str(value, (inner.width as usize).saturating_sub(3 + label.len() + 2)),
                val_style,
            );
        }

        let close_lbl = "[ Close ]";
        let close_w = close_lbl.len() as u16;
        let btn_x = inner.x + (inner.width.saturating_sub(close_w)) / 2;
        let btn_y = inner.y + 5;
        Paragraph::new(close_lbl)
            .style(Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD))
            .render(Rect::new(btn_x, btn_y, close_w, 1), buf);

        Self::draw_shadow(buf, canvas, w, h);
    }

    fn render_info_text(&self, area: Rect, buf: &mut Buffer, text: &str) {
        let w = (area.width.saturating_sub(3)) as usize;
        let lines: Vec<String> = text.split('\n').flat_map(|l| dslbrowser::wrap_text(l, w)).collect();
        let max_off = lines.len().saturating_sub(area.height as usize);
        let off = self.info_scroll.get().min(max_off);
        let body = Style::default().fg(palette::FG);
        for (yy, line) in (area.y..).zip(lines.iter().skip(off).take(area.height as usize)) {
            if yy >= area.bottom() {
                break;
            }
            buf.set_string(area.x + 2, yy, line, body);
        }
        self.draw_scrollbar(buf, area, off, lines.len().max(1), area.height as usize);
    }

    fn render_info_lines(&self, area: Rect, buf: &mut Buffer, lines: &[ratatui::text::Line]) {
        let max_off = lines.len().saturating_sub(area.height as usize);
        let off = self.info_scroll.get().min(max_off);
        for (yy, line) in (area.y..).zip(lines.iter().skip(off).take(area.height as usize)) {
            if yy >= area.bottom() {
                break;
            }
            let spans = line.spans.clone();
            let mut x = area.x + 2;
            for span in spans {
                buf.set_span(x, yy, &span, area.width.saturating_sub(x.saturating_sub(area.x)));
                x += span.width() as u16;
            }
        }
        self.draw_scrollbar(buf, area, off, lines.len().max(1), area.height as usize);
    }

    fn render_placeholder(&self, area: Rect, buf: &mut Buffer, msg: &str, style: &Style) {
        let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
        let y = area.y + area.height / 2;
        buf.set_string(x, y, msg, *style);
    }

    fn render_args_opts(&self, area: Rect, buf: &mut Buffer, items: &[ConsoleModuleArgument], _is_opts: bool) {
        let name_max_w = items.iter().map(|a| a.name.len()).max().unwrap_or(8).min(16);
        let left_w = name_max_w + 2;
        let desc_x = (area.x + 2 + left_w as u16 + 6).max(area.x + 14);
        let desc_w = area.right().saturating_sub(desc_x + 1) as usize;

        let key_style = Style::default().fg(palette::WARNING_PEAK);
        let req_style = Style::default().fg(palette::ERROR_HEAT);
        let opt_style = Style::default().fg(palette::SUCCESS_HEAT);
        let def_style = Style::default().fg(palette::WARNING_GLOW);
        let desc_style = Style::default().fg(palette::GRAY_1);

        #[derive(Clone)]
        struct LineSeg {
            text: String,
            x: u16,
            style: Style,
        }
        let mut all_rows: Vec<Vec<LineSeg>> = Vec::new();

        for item in items {
            let is_req = item.required.unwrap_or(false);
            let tag = if is_req { "required" } else { "optional" };
            let tag_style = if is_req { req_style } else { opt_style };

            // Left column
            let mut left = vec![
                LineSeg { text: item.name.clone(), x: area.x + 2, style: key_style },
                LineSeg { text: tag.to_string(), x: area.x + 2, style: tag_style },
            ];
            if let Some(ref d) = item.default
                && !d.is_empty()
            {
                left.push(LineSeg { text: format!("default: {d}"), x: area.x + 2, style: def_style });
            }

            // Right column (description, wrapped)
            let desc_lines = dslbrowser::wrap_text(&item.description, desc_w);
            let right: Vec<Vec<LineSeg>> = desc_lines.iter().map(|l| vec![LineSeg { text: l.clone(), x: desc_x, style: desc_style }]).collect();

            // Merge: description starts on same line as name
            let rows = left.len().max(right.len());
            for i in 0..rows {
                let mut row = Vec::new();
                if i < left.len() {
                    row.push(left[i].clone());
                }
                if i < right.len() {
                    row.extend(right[i].clone());
                }
                all_rows.push(row);
            }
            // Blank separator
            all_rows.push(Vec::new());
        }

        let total = all_rows.len();
        let view_h = area.height as usize;
        let max_off = total.saturating_sub(view_h);
        let off = self.info_scroll.get().min(max_off);
        for (yy, row) in (area.y..).zip(all_rows.iter().skip(off).take(view_h)) {
            if yy >= area.bottom() {
                break;
            }
            for seg in row {
                buf.set_string(seg.x, yy, &seg.text, seg.style);
            }
        }
        self.draw_scrollbar(buf, area, off, total.max(1), view_h);
    }

    fn draw_scrollbar(&self, buf: &mut Buffer, area: Rect, offset: usize, total: usize, view_h: usize) {
        let bar_h = ((view_h as f64 / total.max(1) as f64) * view_h as f64).max(1.0) as usize;
        let bar_h = bar_h.min(view_h);
        let bar_y = ((offset as f64 / total.max(1) as f64) * (view_h - bar_h) as f64) as usize;
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

fn render_markup_spans(input: &str) -> ratatui::text::Line<'static> {
    use ratatui::text::Span;

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current = String::new();
    let mut style = Style::default();

    fn fg_color(c: char) -> Option<Color> {
        Some(match c {
            'k' => palette::BG_1,
            'r' => palette::ERROR_PEAK,
            'g' => palette::SUCCESS,
            'y' => palette::WARNING,
            'b' => palette::PROCESSING,
            'm' => palette::HIGHLIGHT,
            'c' => palette::SUCCESS_PEAK,
            'w' => palette::FG,
            'K' => palette::GRAY_1,
            'R' => palette::ERROR_GLOW,
            'G' => palette::SUCCESS_GLOW,
            'Y' => palette::WARNING_PEAK,
            'B' => palette::PROCESSING_GLOW,
            'M' => palette::SECONDARY,
            'C' => palette::SECONDARY,
            'W' => palette::FG,
            _ => return None,
        })
    }

    fn bg_color(c: char) -> Option<Color> {
        Some(match c {
            'k' => palette::BG_1,
            'r' => palette::ERROR_BASE,
            'g' => palette::SUCCESS_BASE,
            'y' => palette::WARNING_BASE,
            'b' => palette::PROCESSING_BASE,
            'm' => palette::HIGHLIGHT,
            'c' => palette::SECONDARY,
            'w' => palette::FG,
            _ => return None,
        })
    }

    fn attrs(chars: &str) -> Modifier {
        let mut m = Modifier::empty();
        for c in chars.chars() {
            match c {
                'b' => m |= Modifier::BOLD,
                'd' => m |= Modifier::DIM,
                'u' => m |= Modifier::UNDERLINED,
                'i' => m |= Modifier::REVERSED,
                's' => m |= Modifier::CROSSED_OUT,
                _ => {}
            }
        }
        m
    }

    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '[' {
            current.push(ch);
            continue;
        }
        let mut tag = String::new();
        while let Some(&c) = chars.peek() {
            chars.next();
            if c == ']' {
                break;
            }
            tag.push(c);
        }
        if tag == "N" {
            if !current.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            style = Style::default();
            continue;
        }
        let mut parts = tag.splitn(3, ':');
        let fg = parts.next().unwrap_or("");
        let bg = parts.next().unwrap_or("");
        let at = parts.next().unwrap_or("");
        if !tag.contains(':') {
            current.push('[');
            current.push_str(&tag);
            current.push(']');
            continue;
        }
        if !current.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut current), style));
        }
        if let Some(c) = fg.chars().next()
            && let Some(col) = fg_color(c)
        {
            style = style.fg(col);
        }
        if let Some(c) = bg.chars().next()
            && let Some(col) = bg_color(c)
        {
            style = style.bg(col);
        }
        style = style.add_modifier(attrs(at));
    }
    if !current.is_empty() {
        spans.push(Span::styled(current, style));
    }
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    ratatui::text::Line::from(spans)
}

fn render_module_row(area: Rect, ry: u16, row: &ConsoleModuleRow, sel: bool, name_w: u16, ver_w: u16, buf: &mut Buffer) {
    let hl = Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT);
    let fg = Style::default().fg(palette::FG);
    let ver_fg = Style::default().fg(palette::HIGHLIGHT);
    let desc_fg = Style::default().fg(palette::GRAY_1);
    let row_style = if sel { hl } else { fg };
    if sel {
        for cx in 0..area.width {
            if let Some(cell) = buf.cell_mut(Position::new(area.x + cx, ry)) {
                cell.set_bg(palette::HIGHLIGHT);
            }
        }
    }
    buf.set_string(area.x + 1, ry, format!(" {}", truncate_str(&row.name, name_w as usize)), row_style);
    let ver_style = if sel { row_style } else { ver_fg };
    buf.set_string(area.x + 1 + name_w + 1, ry, truncate_str(row.version.as_deref().unwrap_or("—"), ver_w as usize), ver_style);
    let desc_style = if sel { row_style } else { desc_fg };
    let desc_x = area.x + 1 + name_w + 1 + ver_w + 1;
    let max_desc = (area.width.saturating_sub(name_w + ver_w + 3)) as usize;
    buf.set_string(desc_x, ry, truncate_str(&row.descr, max_desc), desc_style);
}

fn truncate_str(s: &str, max_w: usize) -> String {
    if s.len() <= max_w { s.to_string() } else { format!("{}…", &s[..max_w.saturating_sub(1)]) }
}
