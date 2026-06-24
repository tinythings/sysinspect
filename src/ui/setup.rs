use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use libsysinspect::cfg::mmconf::{MasterConfig, SysInspectConfig};
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, StatefulWidget, Widget},
};
use ratatui_cheese::input::{Input, InputState};
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::dashed_title;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstallationMode {
    SystemWide,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupFocus {
    SysMasterPath,
    SystemRadio,
    CustomRadio,
    CustomDest,
    BindAddr,
    BindPort,
    FsPort,
    ApiCheck,
    Ok,
    Cancel,
}

impl SetupFocus {
    fn next(self, mode: InstallationMode) -> Self {
        use SetupFocus::*;

        match self {
            SysMasterPath => SystemRadio,
            SystemRadio => CustomRadio,
            CustomRadio => {
                if mode == InstallationMode::Custom {
                    CustomDest
                } else {
                    BindAddr
                }
            }
            CustomDest => BindAddr,
            BindAddr => BindPort,
            BindPort => FsPort,
            FsPort => ApiCheck,
            ApiCheck => Ok,
            Ok => Cancel,
            Cancel => SysMasterPath,
        }
    }

    fn prev(self, mode: InstallationMode) -> Self {
        use SetupFocus::*;

        match self {
            SysMasterPath => Cancel,
            SystemRadio => SysMasterPath,
            CustomRadio => SystemRadio,
            CustomDest => CustomRadio,
            BindAddr => {
                if mode == InstallationMode::Custom {
                    CustomDest
                } else {
                    CustomRadio
                }
            }
            BindPort => BindAddr,
            FsPort => BindPort,
            ApiCheck => FsPort,
            Ok => ApiCheck,
            Cancel => Ok,
        }
    }
}

#[derive(Debug)]
pub struct MasterSetupWizard {
    pub visible: bool,
    pub installation_mode: InstallationMode,
    pub sysmaster_path: InputState,
    pub launch_file_picker: bool,
    pub launch_dir_picker: bool,
    pub custom_destination: InputState,
    pub bind_addr: InputState,
    pub bind_port: InputState,
    pub fs_port: InputState,
    pub api_enabled: bool,
    pub focus: SetupFocus,
    pub ok_pressed: bool,
    pub quit_requested: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetupProgress {
    pub visible: bool,
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub done: bool,
    pub error: Option<String>,
}

impl SetupProgress {
    pub fn new() -> Self {
        Self { visible: true, current: 0, total: 0, message: "Scanning setup artifacts...".to_string(), done: false, error: None }
    }

    pub fn hidden() -> Self {
        Self { visible: false, current: 0, total: 0, message: String::new(), done: false, error: None }
    }
}

#[derive(Debug, Clone)]
pub struct SetupCompletion {
    pub info_message: String,
    pub info_styled: Text<'static>,
}

#[derive(Debug, Clone)]
pub struct SetupRequest {
    installation_mode: InstallationMode,
    sysmaster_path: String,
    custom_destination: String,
    bind_addr: String,
    bind_port: String,
    fs_port: String,
    api_enabled: bool,
}

#[derive(Debug, Clone)]
struct SetupCopyEntry {
    source: PathBuf,
    destination: PathBuf,
    label: String,
}

impl Default for MasterSetupWizard {
    fn default() -> Self {
        let cwd = std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

        let mut sysmaster_path = InputState::new();
        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join("sysmaster");
            if candidate.exists() && candidate.is_file() {
                sysmaster_path.set_value(candidate.to_string_lossy().to_string());
            }
        }

        let mut custom_dest = InputState::new();
        custom_dest.set_value(cwd);

        let mut bind_addr = InputState::new();
        bind_addr.set_value("0.0.0.0".to_string());

        let mut bind_port = InputState::new();
        bind_port.set_value("4200".to_string());

        let mut fs_port = InputState::new();
        fs_port.set_value("4201".to_string());

        Self {
            visible: false,
            installation_mode: InstallationMode::SystemWide,
            sysmaster_path,
            launch_file_picker: false,
            launch_dir_picker: false,
            custom_destination: custom_dest,
            bind_addr,
            bind_port,
            fs_port,
            api_enabled: true,
            focus: SetupFocus::SysMasterPath,
            ok_pressed: false,
            quit_requested: false,
            error_message: None,
        }
    }
}

impl MasterSetupWizard {
    pub fn to_request(&self) -> SetupRequest {
        SetupRequest {
            installation_mode: self.installation_mode,
            sysmaster_path: self.sysmaster_path.value().to_string(),
            custom_destination: self.custom_destination.value().to_string(),
            bind_addr: self.bind_addr.value().to_string(),
            bind_port: self.bind_port.value().to_string(),
            fs_port: self.fs_port.value().to_string(),
            api_enabled: self.api_enabled,
        }
    }

    pub fn from_config(cfg: &libsysinspect::cfg::mmconf::MasterConfig) -> Self {
        let root = cfg.root_dir();
        let is_system = root == *"/etc/sysinspect";
        let mut w = MasterSetupWizard {
            installation_mode: if is_system { InstallationMode::SystemWide } else { InstallationMode::Custom },
            ..Default::default()
        };

        // Pre-fill from existing config
        let bind = cfg.bind_addr();
        w.bind_addr.set_value(bind.split(':').next().unwrap_or("0.0.0.0").to_string());
        w.bind_port.set_value(bind.split(':').nth(1).unwrap_or("4200").to_string());

        let fs = cfg.fileserver_bind_addr();
        w.fs_port.set_value(fs.split(':').nth(1).unwrap_or("4201").to_string());

        w.api_enabled = cfg.api_enabled();

        if !is_system {
            w.custom_destination.set_value(root.to_string_lossy().to_string());
        }

        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join("sysmaster");
            if candidate.exists() && candidate.is_file() {
                w.sysmaster_path.set_value(candidate.to_string_lossy().to_string());
            }
        }

        w.focus = SetupFocus::SysMasterPath;
        w
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.visible {
            return false;
        }
        match key.code {
            KeyCode::Tab => {
                self.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus.prev(self.installation_mode)
                } else {
                    self.focus.next(self.installation_mode)
                };
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev(self.installation_mode);
            }
            KeyCode::Enter => match self.focus {
                SetupFocus::SysMasterPath => {
                    self.launch_file_picker = true;
                }
                SetupFocus::Ok => {
                    self.ok_pressed = true;
                }
                SetupFocus::Cancel => {
                    self.quit_requested = true;
                }
                SetupFocus::SystemRadio => {
                    self.installation_mode = InstallationMode::SystemWide;
                }
                SetupFocus::CustomRadio => {
                    self.installation_mode = InstallationMode::Custom;
                }
                SetupFocus::CustomDest => {
                    self.launch_dir_picker = true;
                }
                SetupFocus::ApiCheck => {
                    self.api_enabled = !self.api_enabled;
                }
                _ => {} // input fields — Enter does nothing (text is handled by char keys)
            },
            KeyCode::Esc => {
                self.quit_requested = true;
            }
            KeyCode::Char(' ') => {
                if self.focus == SetupFocus::ApiCheck {
                    self.api_enabled = !self.api_enabled;
                }
            }
            KeyCode::Backspace => {
                if let Some(i) = self.focused_input_mut() {
                    i.delete_before()
                }
            }
            KeyCode::Delete => {
                if let Some(i) = self.focused_input_mut() {
                    i.delete_at()
                }
            }
            KeyCode::Left => {
                if let Some(i) = self.focused_input_mut() {
                    i.move_left()
                }
            }
            KeyCode::Right => {
                if let Some(i) = self.focused_input_mut() {
                    i.move_right()
                }
            }
            KeyCode::Home => {
                if let Some(i) = self.focused_input_mut() {
                    i.home()
                }
            }
            KeyCode::End => {
                if let Some(i) = self.focused_input_mut() {
                    i.end()
                }
            }
            KeyCode::Char(c) => {
                if let Some(i) = self.focused_input_mut() {
                    i.insert_char(c)
                }
            }
            _ => {}
        }
        true
    }

    fn focused_input_mut(&mut self) -> Option<&mut InputState> {
        match self.focus {
            SetupFocus::SysMasterPath => Some(&mut self.sysmaster_path),
            SetupFocus::CustomDest => Some(&mut self.custom_destination),
            SetupFocus::BindAddr => Some(&mut self.bind_addr),
            SetupFocus::BindPort => Some(&mut self.bind_port),
            SetupFocus::FsPort => Some(&mut self.fs_port),
            _ => None,
        }
    }

    fn is_focused(&self, target: SetupFocus) -> bool {
        self.focus == target
    }

    pub fn render(&self, parent: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }
        let dlg_w = (parent.width * 3 / 4).clamp(60, 72);
        let dlg_h = if self.installation_mode == InstallationMode::Custom { 15u16 } else { 14u16 };
        let x = parent.x + (parent.width.saturating_sub(dlg_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(dlg_h)) / 2;
        let canvas = Rect { x, y, width: dlg_w, height: dlg_h };

        Clear.render(canvas, buf);

        let grad = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::GRAY_0, palette::BG_1] as &[Color]);
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
            &[
                TitleSegment { text: " Master ".into(), bg: palette::PROCESSING_GLOW, fg: palette::WHITE, modifier: Modifier::empty() },
                TitleSegment { text: " Setup ".into(), bg: palette::PROCESSING_HEAT, fg: palette::WHITE, modifier: Modifier::empty() },
            ],
        );

        if inner.height < 3 {
            return;
        }

        let label_w = 20u16;
        let focus_style = Style::default().fg(palette::FORM_LABEL_SELECTED).add_modifier(Modifier::BOLD);
        let muted = Style::default().fg(palette::FORM_LABEL);

        let mut row_y = inner.y;

        // ── Installation section ──
        dashed_title(
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
            buf,
            " Installation ",
            palette::PROCESSING,
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        row_y += 1;

        // SysMaster path row
        Self::render_input_row(
            inner.x,
            &mut row_y,
            inner.width,
            buf,
            "Sys Master:",
            &self.sysmaster_path,
            self.is_focused(SetupFocus::SysMasterPath),
            label_w,
        );

        // Destination row with inline radio options
        let sys_checked = self.installation_mode == InstallationMode::SystemWide;
        let sys_style = if self.is_focused(SetupFocus::SystemRadio) { focus_style } else { muted };
        let cus_style = if self.is_focused(SetupFocus::CustomRadio) { focus_style } else { muted };

        let sys_bullet = if sys_checked { "\u{1F518}" } else { "\u{25EF}" };
        let cus_bullet = if sys_checked { "\u{25EF}" } else { "\u{1F518}" };

        let dest_label = format!("{:width$}", "Destination:", width = label_w as usize);
        buf.set_string(inner.x + 2, row_y, &dest_label, muted);
        let sys_text = format!(" {sys_bullet}  System wide (/usr/bin)  ");
        let cus_text = format!(" {cus_bullet}  Custom");
        let label_end = (inner.x + 2) + label_w;
        buf.set_string(label_end, row_y, &sys_text, sys_style);
        buf.set_string(label_end + sys_text.len() as u16, row_y, &cus_text, cus_style);
        row_y += 1;

        // Custom destination row (only when Custom is selected)
        if self.installation_mode == InstallationMode::Custom {
            let cdest_label = "Destination path: ";
            let cdest_lstyle = if self.is_focused(SetupFocus::CustomDest) { focus_style } else { muted };
            buf.set_string(inner.x + 2, row_y, cdest_label, cdest_lstyle);
            let input_x = inner.x + 2 + label_w;
            let input_w = inner.width.saturating_sub(8 + label_w);
            if input_w > 0 {
                let mut is = Self::copy_input_state(&self.custom_destination, self.is_focused(SetupFocus::CustomDest));
                let inp = Input::new("").prompt("").placeholder("path to install root...");
                StatefulWidget::render(&inp, Rect::new(input_x, row_y, input_w, 1), buf, &mut is);
            }
            row_y += 1;
        }

        // spacing
        row_y += 1;

        // ── Configuration section ──
        dashed_title(
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
            buf,
            " Configuration ",
            palette::PROCESSING,
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        row_y += 1;

        // Bind address row
        Self::render_input_row(
            inner.x,
            &mut row_y,
            inner.width,
            buf,
            "Bind address:",
            &self.bind_addr,
            self.is_focused(SetupFocus::BindAddr),
            label_w,
        );
        // Bind port row
        Self::render_input_row(inner.x, &mut row_y, inner.width, buf, "Bind port:", &self.bind_port, self.is_focused(SetupFocus::BindPort), label_w);
        // Fileserver port row
        Self::render_input_row(
            inner.x,
            &mut row_y,
            inner.width,
            buf,
            "Fileserver port:",
            &self.fs_port,
            self.is_focused(SetupFocus::FsPort),
            label_w,
        );

        // API checkbox
        let api_chk = if self.api_enabled { " ▣  Enable Web API" } else { " □  Enable Web API" };
        let api_style = if self.is_focused(SetupFocus::ApiCheck) { focus_style } else { muted };
        buf.set_string(inner.x + 1, row_y, api_chk, api_style);
        row_y += 1;

        // spacing
        row_y += 1;

        // ── Buttons ──
        let ok_label = super::SysInspectUX::format_button("OK");
        let cancel_label = super::SysInspectUX::format_button("Cancel");
        let gap = 3u16;
        let btn_w = ok_label.len() as u16 + gap + cancel_label.len() as u16;
        let btn_x = inner.x + (inner.width.saturating_sub(btn_w)) / 2;

        let ok_style = if self.is_focused(SetupFocus::Ok) {
            Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD)
        };
        let cancel_style = if self.is_focused(SetupFocus::Cancel) {
            Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD)
        };

        buf.set_string(btn_x, row_y, &ok_label, ok_style);
        buf.set_string(btn_x + ok_label.len() as u16 + gap, row_y, &cancel_label, cancel_style);

        if let Some(ref err) = self.error_message {
            let err_y = row_y.saturating_sub(1);
            buf.set_string(inner.x + 2, err_y, err.as_str(), Style::default().fg(palette::ERROR_PEAK));
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

    #[allow(clippy::too_many_arguments)]
    fn render_input_row(
        base_x: u16, row_y: &mut u16, inner_width: u16, buf: &mut Buffer, label: &str, state: &InputState, focused: bool, label_w: u16,
    ) {
        let muted = Style::default().fg(palette::FORM_LABEL);
        let focus_style = Style::default().fg(palette::FORM_LABEL_SELECTED).add_modifier(Modifier::BOLD);
        let lstyle = if focused { focus_style } else { muted };
        let label_padded = format!("{:width$}", label, width = label_w as usize);
        buf.set_string(base_x + 2, *row_y, &label_padded, lstyle);
        let input_x = base_x + 2 + label_w;
        let input_w = inner_width.saturating_sub(label_w + 6);
        if input_w > 0 {
            let mut is = Self::copy_input_state(state, focused);
            let inp = Input::new("").prompt("").placeholder("");
            StatefulWidget::render(&inp, Rect::new(input_x, *row_y, input_w, 1), buf, &mut is);
        }
        *row_y += 1;
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

    /// Write config file, create directories, return the config path on success.
    pub fn write_config(&self) -> Result<std::path::PathBuf, String> {
        self.to_request().run(None).map(|done| {
            let _ = done;
            self.root_dir_for_write_config().join(if matches!(self.installation_mode, InstallationMode::SystemWide) {
                "sysinspect.conf"
            } else {
                "etc/sysinspect.conf"
            })
        })
    }
}

pub fn render_progress(progress: &SetupProgress, parent: Rect, buf: &mut Buffer) {
    if !progress.visible {
        return;
    }

    let dlg_w = (parent.width / 2).clamp(50, 80);
    let dlg_h = 7u16;
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

    let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
    title::overlay_gradient_title(
        buf,
        canvas,
        &title_style,
        &[
            TitleSegment { text: " Master ".into(), bg: palette::PROCESSING_GLOW, fg: palette::WHITE, modifier: Modifier::empty() },
            TitleSegment { text: " Setup ".into(), bg: palette::PROCESSING_HEAT, fg: palette::WHITE, modifier: Modifier::empty() },
        ],
    );

    let msg = if progress.message.is_empty() { "Preparing setup..." } else { &progress.message };
    buf.set_string(inner.x + 1, inner.y + 1, truncate_str(msg, inner.width.saturating_sub(2) as usize), Style::default().fg(palette::FG));

    let bar_y = inner.y + 3;
    let bar_w = inner.width.saturating_sub(2);
    let filled = (bar_w as usize * progress.current).checked_div(progress.total.max(1)).map(|v| v as u16).unwrap_or(0);
    if filled > 0 {
        buf.set_string(inner.x + 1, bar_y, "█".repeat(filled as usize), Style::default().fg(palette::PROCESSING_PEAK));
    }
    if filled < bar_w {
        buf.set_string(inner.x + 1 + filled, bar_y, "─".repeat((bar_w - filled) as usize), Style::default().fg(palette::MUTED));
    }

    let pct = (progress.current * 100).checked_div(progress.total.max(1)).map(|p| format!("{p}%")).unwrap_or_else(|| "0%".into());
    let pct_x = inner.x + (inner.width.saturating_sub(pct.len() as u16)) / 2;
    buf.set_string(pct_x, inner.y + 4, &pct, Style::default().fg(palette::FG).add_modifier(Modifier::BOLD));
}

impl SetupRequest {
    fn total_progress_steps(entry_count: usize) -> usize {
        entry_count.saturating_add(1)
    }

    fn dist_root_from_current_exe() -> Result<PathBuf, String> {
        let exe = std::env::current_exe().map_err(|e| format!("Cannot locate sysinspect binary: {e}"))?;
        exe.parent().and_then(Path::parent).map(Path::to_path_buf).ok_or_else(|| format!("Cannot resolve dist root from {}", exe.display()))
    }

    fn dist_platform_arch_from_current_exe() -> Result<(String, String), String> {
        let dist_root = Self::dist_root_from_current_exe()?;
        let platform = dist_root
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .ok_or_else(|| format!("Cannot resolve platform from {}", dist_root.display()))?;
        let label =
            dist_root.file_name().and_then(|name| name.to_str()).ok_or_else(|| format!("Cannot resolve dist label from {}", dist_root.display()))?;
        let parts: Vec<&str> = label.split('-').collect();
        if parts.len() < 3 {
            return Err(format!("Cannot resolve architecture from dist label {label}"));
        }
        let arch = parts[1..parts.len() - 1].join("-");
        if arch.is_empty() {
            return Err(format!("Cannot resolve architecture from dist label {label}"));
        }
        Ok((platform, arch))
    }

    fn collect_dir_entries(source_root: &Path, dest_root: &Path, label_prefix: &str, entries: &mut Vec<SetupCopyEntry>) -> Result<(), String> {
        let read_dir = std::fs::read_dir(source_root).map_err(|e| format!("Cannot read {}: {e}", source_root.display()))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("Cannot read directory entry in {}: {e}", source_root.display()))?;
            let path = entry.path();
            let name = entry.file_name();
            let dest = dest_root.join(&name);
            let label =
                if label_prefix.is_empty() { name.to_string_lossy().to_string() } else { format!("{label_prefix}/{}", name.to_string_lossy()) };
            if path.is_dir() {
                Self::collect_dir_entries(&path, &dest, &label, entries)?;
            } else if path.is_file() {
                entries.push(SetupCopyEntry { source: path, destination: dest, label });
            }
        }
        Ok(())
    }

    fn collect_dist_entries(&self, root: &Path) -> Result<Vec<SetupCopyEntry>, String> {
        let mut entries = Vec::new();
        let bin_dir = root.join("bin");
        let sysmaster_src = PathBuf::from(&self.sysmaster_path);
        let sysinspect_src = std::env::current_exe().map_err(|e| format!("Cannot locate sysinspect binary: {e}"))?;
        entries.push(SetupCopyEntry { source: sysmaster_src, destination: bin_dir.join("sysmaster"), label: "bin/sysmaster".to_string() });
        entries.push(SetupCopyEntry { source: sysinspect_src, destination: bin_dir.join("sysinspect"), label: "bin/sysinspect".to_string() });

        let dist_root = Self::dist_root_from_current_exe()?;
        let (platform, arch) = Self::dist_platform_arch_from_current_exe()?;
        let payload_root = root.join("dist").join(&platform).join(&arch);
        let minion_src = dist_root.join("bin/sysminion");
        if minion_src.exists() {
            entries.push(SetupCopyEntry {
                source: minion_src,
                destination: payload_root.join("platform").join("sysminion"),
                label: format!("dist/{platform}/{arch}/platform/sysminion"),
            });
        }

        let models_src = dist_root.join("models");
        if models_src.exists() {
            Self::collect_dir_entries(&models_src, &payload_root.join("models"), &format!("dist/{platform}/{arch}/models"), &mut entries)?;
        }

        let modules_src = dist_root.join("modules");
        if modules_src.exists() {
            Self::collect_dir_entries(&modules_src, &payload_root.join("modules"), &format!("dist/{platform}/{arch}/modules"), &mut entries)?;
        }

        Ok(entries)
    }

    fn prepare_copy_entries(&self) -> Result<Vec<SetupCopyEntry>, String> {
        let root = self.root_dir();

        std::fs::create_dir_all(&root).map_err(|e| format!("Cannot create root dir: {e}"))?;
        let telemetry_dir = root.join("telemetry");
        let datastore_dir = root.join("datastore");
        let log_dir = root.join("log");
        std::fs::create_dir_all(&telemetry_dir).map_err(|e| format!("Cannot create telemetry dir: {e}"))?;
        std::fs::create_dir_all(&datastore_dir).map_err(|e| format!("Cannot create datastore dir: {e}"))?;
        std::fs::create_dir_all(&log_dir).map_err(|e| format!("Cannot create log dir: {e}"))?;

        let is_system = matches!(self.installation_mode, InstallationMode::SystemWide);
        let config_dir = if is_system { root.clone() } else { root.join("etc") };
        std::fs::create_dir_all(&config_dir).map_err(|e| format!("Cannot create config dir: {e}"))?;

        if is_system { Ok(Vec::new()) } else { self.collect_dist_entries(&root) }
    }

    fn apply_copy_entries(entries: &[SetupCopyEntry], progress: Option<&Arc<Mutex<SetupProgress>>>) -> Result<(), String> {
        if let Some(progress) = progress
            && let Ok(mut state) = progress.lock()
        {
            state.current = 0;
            state.total = Self::total_progress_steps(entries.len());
            state.message =
                if let Some(first) = entries.first() { format!("Copying {}...", first.label) } else { "Writing sysinspect.conf...".to_string() };
        }

        for (idx, entry) in entries.iter().enumerate() {
            if let Some(parent) = entry.destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
            }
            std::fs::copy(&entry.source, &entry.destination)
                .map_err(|e| format!("Cannot copy {} to {}: {e}", entry.source.display(), entry.destination.display()))?;
            if let Some(progress) = progress
                && let Ok(mut state) = progress.lock()
            {
                state.current = idx + 1;
                state.total = Self::total_progress_steps(entries.len());
                state.message =
                    if idx + 1 < entries.len() { format!("Copying {}...", entries[idx + 1].label) } else { "Writing sysinspect.conf...".to_string() };
            }
        }

        Ok(())
    }

    pub fn run_with_progress(self, progress: Arc<Mutex<SetupProgress>>) -> Result<SetupCompletion, String> {
        self.run(Some(progress))
    }

    fn root_dir(&self) -> PathBuf {
        match self.installation_mode {
            InstallationMode::SystemWide => std::path::PathBuf::from("/etc/sysinspect"),
            InstallationMode::Custom => std::path::PathBuf::from(&self.custom_destination),
        }
    }

    fn installed_sysinspect_path(&self) -> String {
        if self.installation_mode == InstallationMode::Custom {
            self.root_dir().join("bin/sysinspect").display().to_string()
        } else {
            "sysinspect".to_string()
        }
    }

    fn completion_message(&self) -> (String, Text<'static>) {
        let root = self.root_dir().display().to_string();
        let command = format!("{} --ui", self.installed_sysinspect_path());

        let plain = if self.installation_mode == InstallationMode::Custom {
            format!(
                "Master installation files were written successfully. Now please quit\nthis UI and go  to the installation target at:\n\n    {root}\n\nThen run the following command:\n\n    {command}\n\nThat new instance will then ask whether to start master in daemon mode."
            )
        } else {
            format!(
                "Master installation files were written successfully. Now please quit\nthis UI.\n\nThen run the following command:\n\n    {command}\n\nThat new instance will then ask whether to start master in daemon mode."
            )
        };

        let styled = if self.installation_mode == InstallationMode::Custom {
            Text::from(vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Master installation files were written successfully. Now please quit",
                    Style::default().fg(palette::FG),
                )]),
                Line::from(vec![Span::styled("this UI and go  to the installation target at:", Style::default().fg(palette::FG))]),
                Line::from(""),
                Line::from(vec![Span::styled(format!("    {root}"), Style::default().fg(palette::PRIMARY))]),
                Line::from(""),
                Line::from(vec![Span::styled("Then run the following command:", Style::default().fg(palette::FG))]),
                Line::from(""),
                Line::from(vec![Span::styled(format!("    {command}"), Style::default().fg(palette::PRIMARY))]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "That new instance will then ask whether to start master in daemon mode.",
                    Style::default().fg(palette::FG),
                )]),
            ])
        } else {
            Text::from(vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Master installation files were written successfully. Now please quit",
                    Style::default().fg(palette::FG),
                )]),
                Line::from(vec![Span::styled("this UI.", Style::default().fg(palette::FG))]),
                Line::from(""),
                Line::from(vec![Span::styled("Then run the following command:", Style::default().fg(palette::FG))]),
                Line::from(""),
                Line::from(vec![Span::styled(format!("    {command}"), Style::default().fg(palette::PRIMARY))]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "That new instance will then ask whether to start master in daemon mode.",
                    Style::default().fg(palette::FG),
                )]),
            ])
        };

        (plain, styled)
    }

    fn run(self, progress: Option<Arc<Mutex<SetupProgress>>>) -> Result<SetupCompletion, String> {
        let root = self.root_dir();
        let entries = self.prepare_copy_entries()?;

        if let Some(progress) = progress.as_ref()
            && let Ok(mut state) = progress.lock()
        {
            state.current = 0;
            state.total = Self::total_progress_steps(entries.len());
            state.message =
                if let Some(first) = entries.first() { format!("Copying {}...", first.label) } else { "Writing sysinspect.conf...".to_string() };
        }

        let is_system = matches!(self.installation_mode, InstallationMode::SystemWide);
        let config_dir = if is_system { root.clone() } else { root.join("etc") };
        if !is_system {
            Self::apply_copy_entries(&entries, progress.as_ref())?;
        }
        let config_path = config_dir.join("sysinspect.conf");

        let bind_addr = &self.bind_addr;
        let bind_port: u32 = self.bind_port.parse().map_err(|_| "Invalid bind port".to_string())?;
        let fs_port: u32 = self.fs_port.parse().map_err(|_| "Invalid fileserver port".to_string())?;

        let partial = format!(
            "root: \"{}\"\nbind.ip: \"{}\"\nbind.port: {}\nfileserver.bind.ip: \"{}\"\nfileserver.bind.port: {}\nfileserver.models: []\napi.enabled: {}\nlog.stream: \"{}/log/sysmaster.standard.log\"\nlog.errors: \"{}/log/sysmaster.errors.log\"\n",
            root.display(),
            bind_addr,
            bind_port,
            bind_addr,
            fs_port,
            self.api_enabled,
            root.display(),
            root.display(),
        );
        let master_cfg: MasterConfig = serde_yaml::from_str(&partial).map_err(|e| format!("Cannot construct config: {e}"))?;
        let yaml = SysInspectConfig::default().set_master_config(master_cfg).to_yaml();
        std::fs::write(&config_path, yaml).map_err(|e| format!("Cannot write config: {e}"))?;
        if let Some(progress) = progress.as_ref()
            && let Ok(mut state) = progress.lock()
        {
            state.current = state.total;
            state.message = "Setup complete".to_string();
        }

        let (info_message, info_styled) = self.completion_message();

        Ok(SetupCompletion { info_message, info_styled })
    }
}

impl MasterSetupWizard {
    fn root_dir_for_write_config(&self) -> PathBuf {
        match self.installation_mode {
            InstallationMode::SystemWide => PathBuf::from("/etc/sysinspect"),
            InstallationMode::Custom => PathBuf::from(self.custom_destination.value()),
        }
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out = s.chars().take(max - 1).collect::<String>();
    out.push('…');
    out
}
