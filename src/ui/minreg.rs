use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
};
use crate::netadd::{
    artifact::{MinionCatalogue, PlatformId},
    parser::parse_entry,
    types::AddHost,
    workflow::{HostSetup, ProbedHost, SetupContext},
};
use crate::sshprobe::detect::SSHPlatformDetector;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use libsysinspect::cfg::mmconf::MasterConfig;
use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, StatefulWidget, Widget},
};
use ratatui_cheese::input::{Input, InputState};
use ratatui_glamour::color::blend_2d;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use unicode_width::UnicodeWidthStr;

static DEFAULT_PATH: &str = "~/sysinspect";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FormFocus {
    Hostname,
    User,
    Path,
    SudoCheck,
    Ok,
    Cancel,
}

impl FormFocus {
    fn next(self) -> Self {
        match self {
            Self::Hostname => Self::User,
            Self::User => Self::Path,
            Self::Path => Self::SudoCheck,
            Self::SudoCheck => Self::Ok,
            Self::Ok => Self::Cancel,
            Self::Cancel => Self::Hostname,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Hostname => Self::Cancel,
            Self::User => Self::Hostname,
            Self::Path => Self::User,
            Self::SudoCheck => Self::Path,
            Self::Ok => Self::Path,
            Self::Cancel => Self::Ok,
        }
    }
}

/// Pre-flight input form before provisioning begins.
#[derive(Debug)]
pub struct RegistrationForm {
    pub visible: bool,
    pub hostname: InputState,
    pub user: InputState,
    pub path: InputState,
    pub use_sudo: bool,
    pub(crate) focus: FormFocus,
    pub ok_pressed: bool,
}

impl Default for RegistrationForm {
    fn default() -> Self {
        let mut hostname = InputState::new();
        hostname.set_focused(true);

        let mut user = InputState::new();
        if let Some(u) = current_user() {
            user.set_value(u);
        }

        let mut path = InputState::new();
        path.set_value(DEFAULT_PATH.to_string());

        Self { visible: false, hostname, user, path, use_sudo: false, focus: FormFocus::Hostname, ok_pressed: false }
    }
}

impl RegistrationForm {
    /// Render the form popup.
    pub fn render(&self, parent: Rect, buf: &mut ratatui::prelude::Buffer) {
        if !self.visible {
            return;
        }
        let dlg_w = (parent.width * 3 / 4).clamp(54, 66);
        let dlg_h = 11u16;
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
            &[TitleSegment { text: " Register Minion ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() }],
        );

        if inner.height < 3 {
            return;
        }

        let label_w = 14u16;
        let focus_style = Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD);
        let muted = Style::default().fg(palette::MUTED);

        let mut row_y = inner.y + 1;

        Self::render_input_row(inner.x, &mut row_y, inner.width, buf, " Hostname:", &self.hostname, self.focus == FormFocus::Hostname, label_w);
        Self::render_input_row(inner.x, &mut row_y, inner.width, buf, " SSH User:", &self.user, self.focus == FormFocus::User, label_w);
        Self::render_input_row(inner.x, &mut row_y, inner.width, buf, " Path:", &self.path, self.focus == FormFocus::Path, label_w);

        row_y += 1;

        // Sudo checkbox
        let sudo_chk = if self.use_sudo { "[x] Use sudo (wheel)" } else { "[ ] Use sudo (wheel)" };
        let sudo_style = if self.focus == FormFocus::SudoCheck { focus_style } else { muted };
        buf.set_string(inner.x + 3, row_y, sudo_chk, sudo_style);

        row_y += 1;

        let ok_label = "  [   OK   ]  ";
        let cancel_label = "  [ Cancel ]  ";
        let btn_w = ok_label.width() as u16 + cancel_label.width() as u16 + 6;
        let btn_x = inner.x + (inner.width.saturating_sub(btn_w)) / 2;

        let ok_style = if self.focus == FormFocus::Ok {
            Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD)
        };
        let cancel_style = if self.focus == FormFocus::Cancel {
            Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD)
        };

        buf.set_string(btn_x, row_y, ok_label, ok_style);
        buf.set_string(btn_x + ok_label.width() as u16 + 4, row_y, cancel_label, cancel_style);

        draw_shadow(buf, canvas, dlg_w, dlg_h);
    }

    /// Handle keyboard input for the form. Returns true if the event was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.visible {
            return false;
        }
        match key.code {
            KeyCode::Tab => {
                self.focus = if key.modifiers.contains(KeyModifiers::SHIFT) { self.focus.prev() } else { self.focus.next() };
            }
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Esc => {
                self.visible = false;
            }
            KeyCode::Char(' ') => {
                if self.focus == FormFocus::SudoCheck {
                    self.use_sudo = !self.use_sudo;
                }
            }
            KeyCode::Enter => match self.focus {
                FormFocus::Ok => {
                    self.ok_pressed = true;
                }
                FormFocus::Cancel => {
                    self.visible = false;
                }
                FormFocus::SudoCheck => {
                    self.use_sudo = !self.use_sudo;
                }
                _ => {}
            },
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
            FormFocus::Hostname => Some(&mut self.hostname),
            FormFocus::User => Some(&mut self.user),
            FormFocus::Path => Some(&mut self.path),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_input_row(
        base_x: u16, row_y: &mut u16, inner_width: u16, buf: &mut ratatui::prelude::Buffer, label: &str, state: &InputState, focused: bool,
        label_w: u16,
    ) {
        let muted = Style::default().fg(palette::MUTED);
        let focus_style = Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD);
        let lstyle = if focused { focus_style } else { muted };
        let label_padded = format!("{:width$}", label, width = label_w as usize);
        buf.set_string(base_x + 3, *row_y, &label_padded, lstyle);
        let input_x = base_x + 3 + label_w;
        let input_w = inner_width.saturating_sub(label_w + 6);
        if input_w > 0 {
            let mut is = copy_input_state(state, focused);
            let inp = Input::new("").prompt("").placeholder("");
            StatefulWidget::render(&inp, Rect::new(input_x, *row_y, input_w, 1), buf, &mut is);
        }
        *row_y += 1;
    }
}

/// Shared progress state between the UI thread and the background provisioning task.
#[derive(Debug)]
pub struct RegistrationProgress {
    pub visible: bool,
    pub step: usize,
    pub total: usize,
    pub message: String,
    pub host: String,
    pub platform: String,
    pub minion_id: Option<String>,
    pub done: bool,
    pub error: Option<String>,
    pub error_scroll: usize,
    pub cancelled: Arc<AtomicBool>,
}

impl RegistrationProgress {
    /// Create a new progress state with an active registration.
    pub fn new(host: String) -> Self {
        Self {
            visible: true,
            step: 0,
            total: STEP_LABELS.len(),
            message: "Connecting...".into(),
            host,
            platform: String::new(),
            minion_id: None,
            done: false,
            error: None,
            error_scroll: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create an inactive progress placeholder.
    pub fn placeholder() -> Self {
        Self {
            visible: false,
            step: 0,
            total: STEP_LABELS.len(),
            message: String::new(),
            host: String::new(),
            platform: String::new(),
            minion_id: None,
            done: false,
            error: None,
            error_scroll: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Render the progress popup.
pub fn render_progress(progress: &RegistrationProgress, parent: Rect, buf: &mut ratatui::prelude::Buffer) {
    if !progress.visible {
        return;
    }
    let has_error = progress.error.is_some();
    let dlg_w = (parent.width * 3 / 4).clamp(52, 72);
    let dlg_h = if has_error { 16u16 } else { 10u16 };
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
        &[TitleSegment { text: " Minion Registration ".into(), bg: palette::PROCESSING_BASE, fg: palette::FG, modifier: Modifier::empty() }],
    );

    if inner.height < 3 {
        return;
    }

    let mut row_y = inner.y;

    let host_label = format!(" Registering: {}", progress.host);
    buf.set_string(inner.x + 2, row_y, truncate_str(&host_label, (inner.width.saturating_sub(4)) as usize), Style::default().fg(palette::FG));
    row_y += 1;

    if !progress.platform.is_empty() {
        let plat_label = format!(" Detected: {}", progress.platform);
        buf.set_string(inner.x + 2, row_y, truncate_str(&plat_label, (inner.width.saturating_sub(4)) as usize), Style::default().fg(palette::MUTED));
        row_y += 1;
    }

    row_y += 1;

    if let Some(ref err) = progress.error {
        buf.set_string(inner.x + 2, row_y, "Registration failed:", Style::default().fg(palette::ERROR_PEAK).add_modifier(Modifier::BOLD));
        row_y += 1;
        let log_w = inner.width.saturating_sub(5);
        let lines = wrap_text(err, log_w);
        let view_h = (inner.bottom().saturating_sub(row_y + 1)) as usize;
        let max_scroll = lines.len().saturating_sub(view_h);
        let s = progress.error_scroll.min(max_scroll);
        if view_h > 0 {
            let log_start_y = row_y;
            for line in lines.iter().skip(s).take(view_h) {
                buf.set_string(inner.x + 2, row_y, truncate_str(line, log_w as usize), Style::default().fg(palette::MUTED));
                row_y += 1;
            }
            if lines.len() > view_h {
                let sb_x = inner.right().saturating_sub(2);
                for ty in log_start_y..row_y {
                    buf.set_string(sb_x, ty, "│", Style::default().fg(palette::MUTED));
                }
                let thumb_y = (s as f64 / max_scroll.max(1) as f64 * (view_h - 1) as f64) as u16;
                buf.set_string(sb_x, log_start_y + thumb_y, "█", Style::default().fg(palette::PROCESSING_HEAT));
            }
        }
        let close = "[ Close ]";
        let btn_x = inner.x + (inner.width.saturating_sub(close.len() as u16)) / 2;
        buf.set_string(
            btn_x,
            inner.bottom().saturating_sub(1),
            close,
            Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD),
        );
    } else if progress.done {
        let done_msg = if let Some(ref mid) = progress.minion_id { format!(" Registered: {mid}") } else { " Complete".into() };
        buf.set_string(inner.x + 2, row_y, &done_msg, Style::default().fg(palette::SUCCESS_PEAK).add_modifier(Modifier::BOLD));
        row_y += 1;
        let close = "[ Close ]";
        let btn_x = inner.x + (inner.width.saturating_sub(close.len() as u16)) / 2;
        buf.set_string(btn_x, row_y + 1, close, Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD));
    } else {
        let msg = progress.message.clone();
        buf.set_string(inner.x + 2, row_y, truncate_str(&msg, (inner.width.saturating_sub(4)) as usize), Style::default().fg(palette::FG));
        row_y += 1;

        let bar_y = row_y + 1;
        let bar_w = inner.width.saturating_sub(4);
        let pct = (progress.step * 100).checked_div(progress.total.max(1)).unwrap_or(0);
        let filled = (bar_w as usize * progress.step).checked_div(progress.total.max(1)).unwrap_or(0) as u16;

        if filled > 0 {
            buf.set_string(inner.x + 2, bar_y, "█".repeat(filled as usize), Style::default().fg(palette::PROCESSING_PEAK));
        }
        if filled < bar_w {
            let unfilled = (bar_w - filled) as usize;
            buf.set_string(inner.x + 2 + filled, bar_y, "─".repeat(unfilled), Style::default().fg(palette::MUTED));
        }
        let pct_text = format!("{pct}%");
        let pct_x = inner.x + (inner.width.saturating_sub(pct_text.len() as u16)) / 2;
        buf.set_string(pct_x, bar_y, &pct_text, Style::default().fg(palette::FG).add_modifier(Modifier::BOLD));

        let cancel = "[ Cancel ]";
        let btn_x = inner.x + (inner.width.saturating_sub(cancel.len() as u16)) / 2;
        buf.set_string(btn_x, bar_y + 2, cancel, Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD));
    }

    draw_shadow(buf, canvas, dlg_w, dlg_h);
}

/// Handle keyboard input for the progress popup.
pub fn handle_progress_key(key: KeyEvent, progress: &mut RegistrationProgress) -> bool {
    if !progress.visible || progress.done {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter) || key.code == KeyCode::Char(' ') {
            return true;
        }
        return false;
    }
    if let Some(ref _err) = progress.error {
        match key.code {
            KeyCode::Up => {
                progress.error_scroll = progress.error_scroll.saturating_sub(1);
                return false;
            }
            KeyCode::Down => {
                progress.error_scroll += 1;
                return false;
            }
            KeyCode::PageUp => {
                progress.error_scroll = progress.error_scroll.saturating_sub(5);
                return false;
            }
            KeyCode::PageDown => {
                progress.error_scroll += 5;
                return false;
            }
            KeyCode::Esc | KeyCode::Enter => return true,
            _ => {}
        }
        if key.code == KeyCode::Char(' ') {
            return true;
        }
        return false;
    }
    if key.code == KeyCode::Esc {
        progress.cancelled.store(true, Ordering::SeqCst);
        return true;
    }
    false
}

/// Spawn the background provisioning task. Returns a join handle.
pub fn spawn_registration(
    hostname: String, user: String, path: String, use_sudo: bool, cfg: MasterConfig, progress: Arc<Mutex<RegistrationProgress>>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let result = run_provision(hostname, user, path, use_sudo, &cfg, &progress);
        let mut p = progress.lock().unwrap();
        match result {
            Ok(mid) => {
                p.minion_id = mid;
                p.message = "Complete".into();
            }
            Err(err) => {
                p.error = Some(err.to_string());
            }
        }
        p.done = true;
    })
}

/// Execute the full provisioning flow synchronously.
fn run_provision(
    hostname: String, user: String, path: String, use_sudo: bool, cfg: &MasterConfig, progress: &Arc<Mutex<RegistrationProgress>>,
) -> Result<Option<String>, String> {
    let host_display = format!("{user}@{hostname}:{path}");
    {
        let mut p = progress.lock().unwrap();
        p.host = host_display.clone();
    }

    if progress.lock().unwrap().cancelled.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }

    let raw = if path == DEFAULT_PATH { format!("{user}@{hostname}") } else { format!("{user}@{hostname}:{path}") };
    let spec = parse_entry(&raw).map_err(|e| format!("Invalid host entry: {e}"))?;

    let mut p = progress.lock().unwrap();
    if spec.host.trim().is_empty() {
        return Err("Hostname is required".into());
    }
    p.step = 0;
    p.message = STEP_LABELS[0].into();
    drop(p);

    let detector = SSHPlatformDetector::new(&spec.host).set_user(&user).check_writable(true);
    let info = detector.info().map_err(|e| format!("Probe failed: {e}"))?;

    {
        let mut p = progress.lock().unwrap();
        p.platform = info.os_arch();
        if p.cancelled.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        p.step = 1;
        p.message = STEP_LABELS[1].into();
    }

    let ctx = SetupContext::from_cfg(cfg).map_err(|e| format!("Setup context: {e}"))?;
    let host = AddHost {
        raw: spec.raw.clone(),
        host: spec.host.clone(),
        host_norm: spec.host.to_ascii_lowercase(),
        user,
        path: spec.path.clone(),
        path_norm: None,
    };
    let probed = ProbedHost::new(host, info).map_err(|e| format!("Layout: {e}"))?;
    let art = MinionCatalogue::open(ctx.repo_root())
        .map_err(|e| format!("Catalogue: {e}"))?
        .select(&PlatformId::from_probe(&probed.info).map_err(|e| format!("Platform: {e}"))?)
        .map_err(|e| format!("Artefact: {e}"))?;
    let mut setup = HostSetup::new(probed, art);
    setup.set_sudo(use_sudo);

    if progress.lock().unwrap().cancelled.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }

    let progress_clone = Arc::clone(progress);
    let mid = setup
        .run_with_progress(&ctx, move |step, msg| {
            let mut p = progress_clone.lock().unwrap();
            p.step = step + 2;
            p.message = msg.into();
        })
        .map_err(|e| e.to_string())?;

    {
        let mut p = progress.lock().unwrap();
        p.step = p.total;
        p.message = "Complete".into();
    }

    Ok(mid)
}

/// Labels shown in the progress bar in provisioning order.
static STEP_LABELS: &[&str] = &[
    "Probing platform...",
    "Selecting sysminion artefact...",
    "Uploading binary...",
    "Setting permissions...",
    "Verifying binary...",
    "Running setup...",
    "Reading minion ID...",
    "Preparing runtime...",
    "Writing onboarding traits...",
    "Registering with master...",
    "Starting daemon...",
    "Waiting for runtime...",
    "Waiting for bootstrap...",
    "Waiting for readiness...",
    "Syncing CMDB...",
];

fn current_user() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"].into_iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
}

/// Word-wrap text to fit within max_width, preserving line breaks.
fn wrap_text(text: &str, max_w: u16) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut cur = String::new();
        let mut cur_w = 0u16;
        for word in raw_line.split_inclusive(' ') {
            let w = UnicodeWidthStr::width(word) as u16;
            if cur_w + w > max_w && cur_w > 0 {
                lines.push(cur.trim_end().to_string());
                cur = word.to_string();
                cur_w = w;
            } else {
                cur.push_str(word);
                cur_w += w;
            }
        }
        if !cur.is_empty() {
            lines.push(cur.trim_end().to_string());
        }
    }
    lines
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

fn truncate_str(s: &str, max_w: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w <= max_w {
        return s.to_string();
    }
    let mut result = String::with_capacity(max_w + 1);
    let mut cur_w = 0usize;
    for ch in s.chars() {
        let ch_w = UnicodeWidthStr::width(ch.to_string().as_str());
        if cur_w + ch_w > max_w.saturating_sub(1) {
            result.push('…');
            break;
        }
        result.push(ch);
        cur_w += ch_w;
    }
    result
}

fn draw_shadow(buf: &mut ratatui::prelude::Buffer, canvas: Rect, dlg_w: u16, dlg_h: u16) {
    let buf_area = buf.area();
    let max_x = buf_area.right().saturating_sub(1);
    let max_y = buf_area.bottom().saturating_sub(1);
    for idx in 0..dlg_w {
        let sx = canvas.x.saturating_add(2).saturating_add(idx);
        let sy = canvas.y.saturating_add(dlg_h);
        if sx > max_x || sy > max_y {
            continue;
        }
        if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
            cell.set_bg(palette::SHADOW_BG);
            cell.set_fg(palette::SHADOW_FG);
        }
    }
    for off in 0..2u16 {
        for idx in 0..dlg_h {
            let sx = canvas.x.saturating_add(dlg_w).saturating_add(off);
            let sy = canvas.y.saturating_add(idx).saturating_add(1);
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
