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
    widgets::{Block, BorderType, Borders, Clear, StatefulWidget, Widget},
};
use ratatui_cheese::input::{Input, InputState};
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::dashed_title;

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
        let focus_style = Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD);
        let muted = Style::default().fg(palette::MUTED);

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
        let muted = Style::default().fg(palette::MUTED);
        let focus_style = Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD);
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
        let root = match self.installation_mode {
            InstallationMode::SystemWide => std::path::PathBuf::from("/etc/sysinspect"),
            InstallationMode::Custom => std::path::PathBuf::from(self.custom_destination.value()),
        };

        // Create root and subdirs
        std::fs::create_dir_all(&root).map_err(|e| format!("Cannot create root dir: {e}"))?;
        let telemetry_dir = root.join("telemetry");
        let datastore_dir = root.join("datastore");
        let log_dir = root.join("log");
        std::fs::create_dir_all(&telemetry_dir).map_err(|e| format!("Cannot create telemetry dir: {e}"))?;
        std::fs::create_dir_all(&datastore_dir).map_err(|e| format!("Cannot create datastore dir: {e}"))?;
        std::fs::create_dir_all(&log_dir).map_err(|e| format!("Cannot create log dir: {e}"))?;

        // Determine config path and pre-create bin/ for self-contained layouts
        let is_system = matches!(self.installation_mode, InstallationMode::SystemWide);
        let config_dir = if is_system { root.clone() } else { root.join("etc") };
        std::fs::create_dir_all(&config_dir).map_err(|e| format!("Cannot create config dir: {e}"))?;
        if !is_system {
            let bin_dir = root.join("bin");
            std::fs::create_dir_all(&bin_dir).map_err(|e| format!("Cannot create bin dir: {e}"))?;
            let src = std::path::PathBuf::from(self.sysmaster_path.value());
            let dest = bin_dir.join("sysmaster");
            std::fs::copy(&src, &dest).map_err(|e| format!("Cannot copy sysmaster to {}: {e}", dest.display()))?;
            let self_src = std::env::current_exe().map_err(|e| format!("Cannot locate sysinspect binary: {e}"))?;
            let self_dest = bin_dir.join("sysinspect");
            std::fs::copy(&self_src, &self_dest).map_err(|e| format!("Cannot copy sysinspect to {}: {e}", self_dest.display()))?;
        }
        let config_path = config_dir.join("sysinspect.conf");

        let bind_addr = self.bind_addr.value();
        let bind_port: u32 = self.bind_port.value().parse().map_err(|_| "Invalid bind port".to_string())?;
        let fs_port: u32 = self.fs_port.value().parse().map_err(|_| "Invalid fileserver port".to_string())?;

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

        Ok(config_path)
    }
}
