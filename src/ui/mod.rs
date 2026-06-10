use crate::{MEM_LOGGER, call_master_console, ui::elements::DbListItem};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use elements::{ActiveBox, AlertResult, CycleListItem, EventListItem, MinionListItem};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libeventreg::{
    QUERY_CMD_PURGE_ALL, QUERY_CYCLES, QUERY_EVENTS, QUERY_MINIONS,
    ipcc::DbIPCClient,
    kvdb::{EventData, EventMinion, EventSession},
};
use libsysinspect::{
    cfg::mmconf::MasterConfig,
    console::{ConsoleMinionInfoRow, ConsoleModelRow, ConsoleOnlineMinionRow, ConsolePayload},
};
use libsysproto::query::{
    SCHEME_COMMAND,
    commands::{
        CLUSTER_MINION_HOPSTART, CLUSTER_MINION_INFO, CLUSTER_MINION_LOGS, CLUSTER_MINION_RECONNECT, CLUSTER_MINION_SHUTDOWN, CLUSTER_MODELS,
        CLUSTER_ONLINE_MINIONS, CLUSTER_RECONNECT, CLUSTER_SHUTDOWN, CLUSTER_TRAITS_UPDATE,
    },
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row},
};
use ratatui_cheese::tree::TreeState;
use std::{
    cell::{Cell, RefCell},
    io::{self},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use unicode_width::UnicodeWidthStr;

mod alert;
mod dslbrowser;
mod elements;
mod macts;
mod online;
mod palette;
mod rawlogs;
mod setup;
mod statusbar;
mod title;
mod traitsview;
mod traittag;
mod typecolors;
mod wgt;

pub async fn run(cfg: MasterConfig, config_found: bool) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let (result, exit_message) = match SysInspectUX::new(cfg.clone()).await {
        Ok(app) => (app.run_loop(&mut terminal), None),
        Err(err) => {
            if config_found {
                let app = SysInspectUX {
                    cfg,
                    offline: true,
                    error_alert_visible: true,
                    error_alert_message: format!("Cannot connect to master.\n\n{err}\n\nStart the master, then reconnect."),
                    ..Default::default()
                };
                (app.run_offline_loop(&mut terminal), None)
            } else {
                let app = SysInspectUX { cfg, setup_wizard: setup::MasterSetupWizard { visible: true, ..Default::default() }, ..Default::default() };
                let mut exit_message = None;
                let r = app.run_setup_loop(&mut terminal, &mut exit_message);
                (r, exit_message)
            }
        }
    };
    ratatui::restore();

    if let Some(msg) = exit_message {
        println!("\n{msg}\n");
    }

    if !MEM_LOGGER.get_messages().is_empty() {
        println!("Memory log:");
        println!("{:#?}", MEM_LOGGER.get_messages());
    }

    result
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UISizes {
    pub table_cycles: usize,
    pub table_minions: usize,
    pub table_events: usize,
    pub table_info: usize,
}

#[derive(Debug)]
pub struct SysInspectUX {
    exit: bool,
    pub selected_cycle: usize,
    pub selected_minion: usize,
    pub selected_event: usize,

    pub li_minions: Vec<MinionListItem>,
    pub li_events: Vec<EventListItem>,
    pub event_data: IndexMap<String, String>,
    pub active_box: ActiveBox,
    saved_active_box: Option<ActiveBox>,
    no_focus: bool,

    pub status_text: Line<'static>,

    /// Purge alert
    pub purge_alert_visible: bool,
    pub purge_alert_choice: AlertResult,

    /// Error alert
    pub error_alert_visible: bool,
    pub error_alert_message: String,
    pub error_alert_choice: AlertResult,

    /// Information alert (success/info popups)
    pub info_alert_visible: bool,
    pub info_alert_message: String,

    /// Exit alert
    pub exit_alert_visible: bool,
    pub exit_alert_choice: AlertResult,

    // Help popup
    pub help_popup_visible: bool,

    // Online minions popup
    pub minions_visible: bool,
    pub minions_rows: Vec<ConsoleOnlineMinionRow>,
    pub minions_online_sel: usize,
    pub minions_offline_sel: usize,
    pub minions_filter_input: ratatui_cheese::input::InputState,
    pub minions_focus: usize,
    pub minion_traits_visible: bool,
    pub minion_traits_rows: Vec<ConsoleMinionInfoRow>,
    pub minion_traits_tree_state: Option<TreeState>,
    pub minion_traits_modified: bool,
    pub minion_traits_filter: ratatui_cheese::input::InputState,
    pub minion_traits_filter_focus: bool,

    // Raw minion logs popup
    pub minion_logs_visible: bool,
    pub minion_logs_lines: Vec<String>,
    pub minion_logs_scroll: usize,
    pub minion_logs_path: String,
    pub minion_logs_host: String,
    pub minion_logs_source_kind: String,
    pub minion_logs_filter: ratatui_cheese::input::InputState,
    pub minion_logs_filter_focus: bool,
    pub minion_logs_polling: bool,
    pub minion_logs_online: bool,
    pub minion_logs_last_fetch: Instant,
    pub minion_logs_viewport_rows: Cell<usize>,

    // Online minions action menu
    pub minions_menu_visible: bool,
    pub minions_menu_sel: usize,

    // Cluster-wide operation confirmation
    pub cluster_confirm_visible: bool,
    pub cluster_confirm_choice: AlertResult,
    pub pending_cluster_action: u8, // 0=none, 1=shutdown all, 2=reconnect all, 3=delete minion

    // Tag popup
    pub tag_visible: bool,
    pub tag_key_buf: String,
    pub tag_val_buf: String,
    pub tag_focus: u8,
    pub tag_pos: usize,

    // DB
    pub evtipc: Option<Arc<Mutex<DbIPCClient>>>,

    // Config
    /// DSL browser / call composer
    pub dsl_browser: dslbrowser::DslBrowser,

    pub cfg: MasterConfig,

    // Master setup wizard (first-run)
    pub setup_wizard: setup::MasterSetupWizard,

    // Connection state
    pub offline: bool,
    pub last_reconnect_attempt: Instant,

    // Exit-after-popup state (for setup config-written notice)
    pub pending_exit: bool,
    pub pending_exit_message: Option<String>,

    // Buffers
    pub cycles_buf: Vec<CycleListItem>,
    pub minions_buf: Vec<MinionListItem>,
    pub events_buf: Vec<EventListItem>,

    actdt_info_offset: usize,
    info_rows: RefCell<Vec<Row<'static>>>,
    pub size: Cell<UISizes>,
}

impl Default for SysInspectUX {
    fn default() -> Self {
        let mut instance = Self {
            exit: false,
            selected_cycle: 0,
            selected_minion: 0,
            selected_event: 0,
            li_minions: Vec::new(),
            li_events: Vec::new(),
            event_data: IndexMap::new(),
            active_box: ActiveBox::default(),
            saved_active_box: None,
            no_focus: false,
            status_text: Line::from(vec![]),

            // Alerts
            purge_alert_visible: false,
            purge_alert_choice: AlertResult::default(),
            exit_alert_visible: false,
            exit_alert_choice: AlertResult::default(),
            error_alert_visible: false,
            error_alert_choice: AlertResult::default(),
            error_alert_message: String::new(),
            info_alert_visible: false,
            info_alert_message: String::new(),
            help_popup_visible: false,

            minions_visible: false,
            minions_rows: Vec::new(),
            minions_online_sel: 0,
            minions_offline_sel: 0,
            minions_filter_input: ratatui_cheese::input::InputState::new(),
            minions_focus: 1,
            minion_traits_visible: false,
            minion_traits_rows: Vec::new(),
            minion_traits_tree_state: None,
            minion_traits_modified: false,
            minion_traits_filter: ratatui_cheese::input::InputState::new(),
            minion_traits_filter_focus: false,

            minion_logs_visible: false,
            minion_logs_lines: Vec::new(),
            minion_logs_scroll: 0,
            minion_logs_path: String::new(),
            minion_logs_host: String::new(),
            minion_logs_source_kind: String::new(),
            minion_logs_filter: ratatui_cheese::input::InputState::new(),
            minion_logs_filter_focus: false,
            minion_logs_polling: true,
            minion_logs_online: true,
            minion_logs_last_fetch: Instant::now(),
            minion_logs_viewport_rows: Cell::new(0),

            minions_menu_visible: false,
            minions_menu_sel: 0,

            cluster_confirm_visible: false,
            cluster_confirm_choice: AlertResult::default(),
            pending_cluster_action: 0,

            tag_visible: false,
            tag_key_buf: String::new(),
            tag_val_buf: String::new(),
            tag_focus: 0,
            tag_pos: 0,

            evtipc: None,
            dsl_browser: dslbrowser::DslBrowser::new(),
            cfg: MasterConfig::default(),
            setup_wizard: setup::MasterSetupWizard::default(),
            offline: false,
            last_reconnect_attempt: Instant::now(),

            pending_exit: false,
            pending_exit_message: None,
            cycles_buf: Vec::new(),
            minions_buf: Vec::new(),
            events_buf: Vec::new(),

            actdt_info_offset: 0,
            info_rows: RefCell::new(vec![]),
            size: Cell::new(UISizes::default()),
        };
        instance.status_at_cycles(); // Also an initial status
        instance
    }
}

impl SysInspectUX {
    #[allow(clippy::field_reassign_with_default)]
    pub async fn new(cfg: MasterConfig) -> Result<Self, SysinspectError> {
        let mut ux = SysInspectUX::default();

        ux.evtipc = Some(Arc::new(Mutex::new(DbIPCClient::new(cfg.telemetry_socket().to_str().unwrap_or_default()).await?)));
        ux.cfg = cfg;

        Ok(ux)
    }

    pub fn run_loop(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.cycles_buf = self.get_cycles().unwrap_or_default();
        self.run_normal_loop(term)
    }

    pub fn run_setup_loop(mut self, term: &mut DefaultTerminal, exit_msg: &mut Option<String>) -> io::Result<()> {
        self.last_reconnect_attempt = Instant::now();
        self.no_focus = true;

        while !self.exit {
            term.draw(|frame| self.draw(frame))?;
            if self.setup_wizard.ok_pressed {
                match self.setup_wizard.write_config() {
                    Ok(config_path) => {
                        self.setup_wizard.ok_pressed = false;
                        self.setup_wizard.visible = false;
                        let msg = format!(
                            "Config written to:\n{}\n\nStart the master with:\n  sysmaster --start -c {}",
                            config_path.display(),
                            config_path.display(),
                        );
                        self.info_alert_visible = true;
                        self.info_alert_message = msg.clone();
                        self.pending_exit = true;
                        self.pending_exit_message = Some(msg);
                    }
                    Err(e) => {
                        self.setup_wizard.error_message = Some(e);
                        self.setup_wizard.ok_pressed = false;
                    }
                }
            }
            if !self.error_alert_visible && !self.setup_wizard.visible && self.try_reconnect_silent().is_ok() {
                return self.run_normal_loop(term);
            }
            // Periodic silent reconnect in setup mode
            if !self.setup_wizard.ok_pressed && self.last_reconnect_attempt.elapsed() >= Duration::from_secs(5) && self.evtipc.is_some() {
                self.last_reconnect_attempt = Instant::now();
                if self.try_reconnect_silent().is_ok() {
                    return self.run_normal_loop(term);
                }
            }
            self.on_events_setup()?;
        }
        *exit_msg = self.pending_exit_message.take();
        Ok(())
    }

    fn run_normal_loop(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.last_reconnect_attempt = Instant::now();
        while !self.exit {
            self.sync_main_focus_for_overlays();
            term.draw(|frame| self.draw(frame))?;
            self.on_events()?;
            if self.offline && self.last_reconnect_attempt.elapsed() >= Duration::from_secs(5) {
                self.last_reconnect_attempt = Instant::now();
                if self.try_reconnect_silent().is_ok() {
                    self.offline = false;
                }
            }
        }
        Ok(())
    }

    pub fn run_offline_loop(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.last_reconnect_attempt = Instant::now();
        self.no_focus = true;

        while !self.exit {
            term.draw(|frame| self.draw(frame))?;
            // Periodic silent reconnect attempt
            if self.last_reconnect_attempt.elapsed() >= Duration::from_secs(5) {
                self.last_reconnect_attempt = Instant::now();
                if self.try_reconnect_silent().is_ok() {
                    return self.run_normal_loop(term);
                }
            }
            self.on_events()?;
        }
        Ok(())
    }

    fn try_reconnect_silent(&mut self) -> Result<(), String> {
        let socket = self.cfg.telemetry_socket();
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { DbIPCClient::new(socket.to_str().unwrap_or_default()).await })
        }) {
            Ok(ipc) => {
                self.evtipc = Some(Arc::new(Mutex::new(ipc)));
                self.setup_wizard.visible = false;
                self.error_alert_visible = false;
                self.offline = false;
                self.cycles_buf = self.get_cycles().unwrap_or_default();
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn try_reconnect(&mut self) -> Result<(), String> {
        self.try_reconnect_silent().map_err(|e| {
            self.error_alert_visible = true;
            self.error_alert_message = format!("Master still not reachable: {e}");
            e
        })
    }

    fn on_events_setup(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_secs(1))?
            && let Event::Key(e) = event::read()?
            && e.kind == KeyEventKind::Press
        {
            if self.setup_wizard.visible && !self.error_alert_visible && !self.exit_alert_visible && !self.info_alert_visible {
                self.setup_wizard.handle_key(e);
                if self.setup_wizard.quit_requested {
                    self.setup_wizard.quit_requested = false;
                    self.exit_alert_visible = true;
                    self.exit_alert_choice = AlertResult::Default;
                }
            } else {
                self.on_key(e);
            }
        }
        Ok(())
    }

    /// Redraw the screen on every event
    fn draw(&self, frame: &mut Frame) {
        // Split the entire area into main UI and a one-line status bar.
        let chunks =
            Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0), Constraint::Length(1)].as_ref()).split(frame.area());
        let main_area = chunks[0];
        let status_area = chunks[1];

        frame.render_widget(self, main_area);

        if self.offline {
            let offline_w: u16 = 14;
            let [main_status, offline_area]: [Rect; 2] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(offline_w)].as_ref())
                .split(status_area)
                .as_ref()
                .try_into()
                .unwrap();

            let status_paragraph = Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
            frame.render_widget(status_paragraph, main_status);

            let offline_paragraph = Paragraph::new(Line::from(vec![Span::styled(
                "  Offline \u{2716} ",
                Style::default().fg(palette::ERROR_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(palette::BG_1));
            frame.render_widget(offline_paragraph, offline_area);
        } else {
            let status_paragraph = Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
            frame.render_widget(status_paragraph, status_area);
        }
    }

    fn on_events(&mut self) -> io::Result<()> {
        self.sync_main_focus_for_overlays();
        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(e) = event::read()?
                && e.kind == KeyEventKind::Press
            {
                self.on_key(e);
            }
        } else {
            if !self.offline {
                match self.get_cycles() {
                    Ok(cycles) => self.cycles_buf = cycles,
                    Err(_) => {
                        self.offline = true;
                        self.evtipc = None;
                    }
                }
                if !self.offline {
                    if self.minions_visible {
                        self.refresh_minions();
                    }
                    if self.minion_logs_visible && self.minion_logs_polling && self.minion_logs_last_fetch.elapsed() >= Duration::from_secs(3) {
                        match self.load_selected_minion_logs() {
                            Ok(()) => self.minion_logs_online = true,
                            Err(_) => self.minion_logs_online = false,
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Cycle active pan to the right (used on RIGHT or ENTER key)
    fn shift_next(&mut self) {
        if self.no_focus {
            return;
        }
        match self.active_box {
            ActiveBox::Cycles => {
                if self.li_minions.is_empty() {
                    return;
                }
                self.status_at_minions();
                self.active_box = ActiveBox::Minions
            }

            ActiveBox::Minions => {
                if self.li_events.is_empty() {
                    return;
                }
                self.status_at_action_results();
                self.active_box = ActiveBox::Events
            }

            ActiveBox::Events | ActiveBox::Info => {
                self.status_at_cycles();
                self.active_box = ActiveBox::Cycles
            }
        };
    }

    /// Cycle active pan to the left (used on LEFT or ESC key)
    fn shift_prev(&mut self) {
        if self.no_focus {
            return;
        }
        match self.active_box {
            ActiveBox::Cycles => {
                if self.li_minions.is_empty() {
                    return;
                }

                self.status_at_action_results();
                self.active_box = ActiveBox::Events
            }

            ActiveBox::Minions => {
                self.status_at_cycles();
                self.active_box = ActiveBox::Cycles
            }

            ActiveBox::Events | ActiveBox::Info => {
                self.status_at_minions();
                self.active_box = ActiveBox::Minions
            }
        };
    }

    fn on_help_popup(&mut self, e: event::KeyEvent) -> bool {
        let mut stat = false;
        if self.help_popup_visible {
            stat = true;
            match e.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.help_popup_visible = false;
                }
                _ => {}
            }
        }

        stat
    }

    /// Process purge alert key events
    fn on_purge_alert(&mut self, e: event::KeyEvent) -> bool {
        let mut stat = false;
        if self.purge_alert_visible {
            stat = true;
            match e.code {
                KeyCode::Tab => {
                    if self.purge_alert_choice == AlertResult::Default {
                        self.purge_alert_choice = AlertResult::Purge;
                    } else {
                        self.purge_alert_choice = AlertResult::Default;
                    }
                }
                KeyCode::Enter => {
                    if self.purge_alert_choice == AlertResult::Purge {
                        self.purge_database().unwrap_or_else(|err| {
                            self.error_alert_visible = true;
                            self.error_alert_message = err.to_string();
                        });
                    }
                    self.purge_alert_visible = false;
                }
                KeyCode::Esc => {
                    self.purge_alert_visible = false;
                }
                _ => {}
            }
        }

        stat
    }

    /// Process exit alert
    fn on_exit_alert(&mut self, e: event::KeyEvent) -> bool {
        let mut stat = false;
        if self.exit_alert_visible {
            stat = true;
            match e.code {
                KeyCode::Tab => {
                    if self.exit_alert_choice == AlertResult::Default {
                        self.exit_alert_choice = AlertResult::Quit;
                    } else {
                        self.exit_alert_choice = AlertResult::Default;
                    }
                }
                KeyCode::Enter => {
                    if self.exit_alert_choice == AlertResult::Quit {
                        self.exit();
                    } else {
                        self.exit_alert_visible = false;
                    }
                }
                KeyCode::Esc => {
                    self.exit_alert_visible = false;
                }
                _ => {}
            }
        }

        stat
    }

    fn on_error_alert(&mut self, e: event::KeyEvent) -> bool {
        let mut stat = false;
        if self.error_alert_visible {
            stat = true;
            match e.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.error_alert_visible = false;
                }
                _ => {}
            }
        }

        stat
    }

    fn on_info_alert(&mut self, e: event::KeyEvent) -> bool {
        if !self.info_alert_visible {
            return false;
        }
        if matches!(e.code, KeyCode::Enter | KeyCode::Esc) {
            self.info_alert_visible = false;
            if self.pending_exit {
                self.pending_exit = false;
                self.exit();
            }
        }
        true
    }

    /// Process online minions popup key events
    fn on_minions_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.minions_visible {
            return false;
        }

        if self.minion_logs_visible {
            return self.on_minion_logs_popup(e);
        }

        if self.minion_traits_visible {
            return self.on_minion_traits_popup(e);
        }

        if self.minions_menu_visible {
            return self.on_minions_menu(e);
        }

        if !self.minion_logs_visible && !self.minion_traits_visible && self.dispatch_minion_shortcut(&e) {
            return true;
        }

        match self.minions_focus {
            0 => self.on_minions_filter(e),
            _ => self.on_minions_panes(e),
        };
        true
    }

    fn on_minion_traits_popup(&mut self, e: event::KeyEvent) -> bool {
        if self.minion_traits_filter_focus {
            match e.code {
                KeyCode::Esc => {
                    self.minion_traits_filter_focus = false;
                }
                KeyCode::Tab => {
                    let groups = self.minion_traits_groups_filtered();
                    if groups.is_empty() {
                        return true;
                    }
                    self.minion_traits_filter_focus = false;
                    Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                }
                KeyCode::Backspace => {
                    self.minion_traits_filter.delete_before();
                    self.minion_traits_tree_state = None;
                }
                KeyCode::Delete => {
                    self.minion_traits_filter.delete_at();
                    self.minion_traits_tree_state = None;
                }
                KeyCode::Left => {
                    self.minion_traits_filter.move_left();
                }
                KeyCode::Right => {
                    self.minion_traits_filter.move_right();
                }
                KeyCode::Home => {
                    self.minion_traits_filter.home();
                }
                KeyCode::End => {
                    self.minion_traits_filter.end();
                }
                KeyCode::Char(c) => {
                    self.minion_traits_filter.insert_char(c);
                    self.minion_traits_tree_state = None;
                }
                _ => {}
            }
            return true;
        }
        match e.code {
            KeyCode::Esc => {
                self.minion_traits_visible = false;
                self.status_at_minions_browser();
            }
            KeyCode::Tab => {
                self.minion_traits_filter_focus = true;
            }
            KeyCode::Enter => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    let (group, _) = ts.selected();
                    if ts.is_expanded(group) {
                        ts.collapse(group);
                    } else {
                        ts.expand(group);
                    }
                }
            }
            KeyCode::Up => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    ts.select_prev(&groups);
                }
            }
            KeyCode::Down => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    ts.select_next(&groups);
                }
            }
            KeyCode::PageUp => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    for _ in 0..10 {
                        ts.select_prev(&groups);
                    }
                }
            }
            KeyCode::PageDown => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    for _ in 0..10 {
                        ts.select_next(&groups);
                    }
                }
            }
            KeyCode::Left => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    let (group, _) = ts.selected();
                    ts.collapse(group);
                }
            }
            KeyCode::Right => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    let (group, _) = ts.selected();
                    ts.expand(group);
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    for i in 0..groups.len() {
                        ts.expand(i);
                    }
                }
            }
            KeyCode::Char('-') => {
                let groups = self.minion_traits_groups_filtered();
                Self::ensure_info_tree_state_mut(&mut self.minion_traits_tree_state, &groups);
                if let Some(ref mut ts) = self.minion_traits_tree_state {
                    for i in 0..groups.len() {
                        ts.collapse(i);
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn minions_menu_len() -> usize {
        macts::total_menu_items()
    }

    fn on_minions_menu(&mut self, e: event::KeyEvent) -> bool {
        match e.code {
            KeyCode::Esc => {
                self.minions_menu_visible = false;
                self.status_at_minions_browser();
            }
            KeyCode::Up => {
                self.minions_menu_sel = self.minions_menu_sel.saturating_sub(1);
            }
            KeyCode::Down => {
                self.minions_menu_sel = (self.minions_menu_sel + 1).min(Self::minions_menu_len().saturating_sub(1));
            }
            KeyCode::Enter => {
                self.minions_menu_visible = false;
                if self.minions_menu_sel == 0 {
                    self.open_logs_popup();
                } else if self.minions_menu_sel == 1 {
                    self.open_traits_popup();
                } else if self.minions_menu_sel == 2 {
                    self.do_minion_start();
                } else if self.minions_menu_sel == 3 {
                    self.do_minion_shutdown();
                } else if self.minions_menu_sel == 4 {
                    self.do_minion_reconnect();
                } else if self.minions_menu_sel == 5 {
                    self.cluster_confirm_visible = true;
                    self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                    self.pending_cluster_action = 3;
                } else if self.minions_menu_sel == 6 {
                    self.cluster_confirm_visible = true;
                    self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                    self.pending_cluster_action = 1;
                } else if self.minions_menu_sel == 7 {
                    self.cluster_confirm_visible = true;
                    self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                    self.pending_cluster_action = 2;
                } else if self.minions_menu_sel == 8 {
                    // TODO: do_minion_add()
                }
            }
            _ => {
                if self.dispatch_minion_shortcut(&e) {
                    self.minions_menu_visible = false;
                }
            }
        }
        true
    }

    fn minion_traits_groups_filtered(&self) -> Vec<ratatui_cheese::tree::TreeGroup> {
        Self::build_filtered_tree_groups(&self.minion_traits_rows, &self.minion_traits_filter.value().to_lowercase())
    }

    fn build_filtered_tree_groups(rows: &[ConsoleMinionInfoRow], filter: &str) -> Vec<ratatui_cheese::tree::TreeGroup> {
        let filtered: Vec<ConsoleMinionInfoRow> =
            rows.iter().filter(|r| filter.is_empty() || SysInspectUX::_info_value_str(&r.value).to_lowercase().contains(filter)).cloned().collect();
        SysInspectUX::build_info_tree(&filtered)
    }

    fn ensure_info_tree_state_mut(state: &mut Option<TreeState>, groups: &[ratatui_cheese::tree::TreeGroup]) {
        if state.is_none() {
            let mut ts = TreeState::new(groups.len());
            for i in 0..groups.len() {
                ts.expand(i);
            }
            *state = Some(ts);
        }
    }

    fn on_minions_filter(&mut self, e: event::KeyEvent) {
        match e.code {
            KeyCode::Esc => {
                self.minions_visible = false;
                self.restore_status();
            }
            KeyCode::Tab => {
                self.minions_focus = 1;
            }
            KeyCode::BackTab => {
                self.minions_focus = 2;
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Enter => {
                self.minions_focus = 1;
            }
            KeyCode::Backspace => {
                self.minions_filter_input.delete_before();
            }
            KeyCode::Delete => {
                self.minions_filter_input.delete_at();
            }
            KeyCode::Left => {
                self.minions_filter_input.move_left();
            }
            KeyCode::Right => {
                self.minions_filter_input.move_right();
            }
            KeyCode::Home => {
                self.minions_filter_input.home();
            }
            KeyCode::End => {
                self.minions_filter_input.end();
            }
            KeyCode::Char(c) => {
                self.minions_filter_input.insert_char(c);
            }
            _ => {}
        }
    }

    fn on_minions_panes(&mut self, e: event::KeyEvent) {
        if e.code == KeyCode::Char('t') {
            if self.minions_focus != 1 {
                self.error_alert_visible = true;
                self.error_alert_message = "Offline minions cannot be tagged live. Do it manually.".to_string();
            } else {
                let filtered = self.filtered_online();
                if filtered.is_empty() || filtered.len() <= self.minions_online_sel {
                    self.error_alert_visible = true;
                    self.error_alert_message = "No online minion selected.".to_string();
                } else {
                    self.tag_key_buf.clear();
                    self.tag_val_buf.clear();
                    self.tag_focus = 0;
                    self.tag_pos = 0;
                    self.tag_visible = true;
                }
            }
            return;
        }
        match e.code {
            KeyCode::Esc => {
                self.minions_visible = false;
                self.restore_status();
            }
            KeyCode::Tab => {
                self.minions_focus = (self.minions_focus + 1) % 3;
            }
            KeyCode::BackTab => {
                self.minions_focus = (self.minions_focus + 2) % 3;
            }
            KeyCode::Left => {
                self.minions_focus = 1;
            }
            KeyCode::Right => {
                self.minions_focus = 2;
            }
            KeyCode::Up => match self.minions_focus {
                1 => {
                    self.minions_online_sel = self.minions_online_sel.saturating_sub(1);
                    self.minion_traits_modified = false;
                }
                2 => {
                    self.minions_offline_sel = self.minions_offline_sel.saturating_sub(1);
                    self.minion_traits_modified = false;
                }
                _ => {}
            },
            KeyCode::Down => match self.minions_focus {
                1 => {
                    let filtered = self.filtered_online();
                    self.minions_online_sel = (self.minions_online_sel + 1).min(filtered.len().saturating_sub(1));
                    self.minion_traits_modified = false;
                }
                2 => {
                    let filtered = self.filtered_offline();
                    self.minions_offline_sel = (self.minions_offline_sel + 1).min(filtered.len().saturating_sub(1));
                    self.minion_traits_modified = false;
                }
                _ => {}
            },
            KeyCode::Enter => {
                self.minions_menu_visible = true;
                self.minions_menu_sel = 0;
                self.status_at_minion_menu();
            }
            KeyCode::PageUp => match self.minions_focus {
                1 => {
                    self.minions_online_sel = self.minions_online_sel.saturating_sub(10);
                    self.minion_traits_modified = false;
                }
                2 => {
                    self.minions_offline_sel = self.minions_offline_sel.saturating_sub(10);
                    self.minion_traits_modified = false;
                }
                _ => {}
            },
            KeyCode::PageDown => match self.minions_focus {
                1 => {
                    let filtered = self.filtered_online();
                    self.minions_online_sel = (self.minions_online_sel + 10).min(filtered.len().saturating_sub(1));
                    self.minion_traits_modified = false;
                }
                2 => {
                    let filtered = self.filtered_offline();
                    self.minions_offline_sel = (self.minions_offline_sel + 10).min(filtered.len().saturating_sub(1));
                    self.minion_traits_modified = false;
                }
                _ => {}
            },
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Home | KeyCode::End => {
                self.minions_focus = 0;
            }
            KeyCode::Char(c) => {
                self.minions_filter_input.insert_char(c);
                self.minions_focus = 0;
            }
            _ => {}
        }
    }

    fn filtered_online(&self) -> Vec<&ConsoleOnlineMinionRow> {
        let f = self.minions_filter_input.value().to_lowercase();
        self.minions_rows.iter().filter(|r| r.alive && (f.is_empty() || SysInspectUX::online_host(r).to_lowercase().contains(&f))).collect()
    }

    fn filtered_offline(&self) -> Vec<&ConsoleOnlineMinionRow> {
        let f = self.minions_filter_input.value().to_lowercase();
        self.minions_rows.iter().filter(|r| !r.alive && (f.is_empty() || SysInspectUX::online_host(r).to_lowercase().contains(&f))).collect()
    }

    fn restore_status(&mut self) {
        match self.active_box {
            ActiveBox::Cycles => self.status_at_cycles(),
            ActiveBox::Minions => self.status_at_minions(),
            ActiveBox::Events => self.status_at_action_results(),
            ActiveBox::Info => self.status_at_action_data(),
        }
    }

    fn any_overlay_visible(&self) -> bool {
        self.purge_alert_visible
            || self.error_alert_visible
            || self.exit_alert_visible
            || self.help_popup_visible
            || self.minions_visible
            || self.minion_traits_visible
            || self.minion_logs_visible
            || self.minions_menu_visible
            || self.tag_visible
            || self.dsl_browser.visible
    }

    fn sync_main_focus_for_overlays(&mut self) {
        let overlay_visible = self.any_overlay_visible();
        if overlay_visible && !self.no_focus {
            self.saved_active_box = Some(self.active_box);
            self.no_focus = true;
        } else if !overlay_visible && self.no_focus {
            self.active_box = self.saved_active_box.take().unwrap_or_default();
            self.no_focus = false;
        }
    }

    pub(crate) fn main_box_active(&self, hl: ActiveBox) -> bool {
        !self.no_focus && self.active_box == hl
    }

    fn refresh_minions(&mut self) {
        if let Ok(rows) = self.fetch_minions() {
            let old_online_mid = self.filtered_online().get(self.minions_online_sel).map(|r| r.minion_id.clone());
            let old_offline_mid = self.filtered_offline().get(self.minions_offline_sel).map(|r| r.minion_id.clone());
            self.minions_rows = rows;
            if let Some(mid) = old_online_mid {
                let filtered = self.filtered_online();
                if let Some(pos) = filtered.iter().position(|r| r.minion_id == mid) {
                    self.minions_online_sel = pos;
                } else {
                    self.minions_online_sel = 0;
                }
            }
            if let Some(mid) = old_offline_mid {
                let filtered = self.filtered_offline();
                if let Some(pos) = filtered.iter().position(|r| r.minion_id == mid) {
                    self.minions_offline_sel = pos;
                } else {
                    self.minions_offline_sel = 0;
                }
            }
        }
    }

    fn load_selected_minion_info(&mut self) {
        if self.minion_traits_modified {
            return;
        }
        let row = match self.minions_focus {
            1 => self.filtered_online().get(self.minions_online_sel).cloned(),
            2 => self.filtered_offline().get(self.minions_offline_sel).cloned(),
            _ => None,
        };
        if let Some(row) = row {
            let expanded: Vec<String> = if let Some(ref ts) = self.minion_traits_tree_state {
                let old_groups = SysInspectUX::build_info_tree(&self.minion_traits_rows);
                old_groups.iter().enumerate().filter(|(i, _)| ts.is_expanded(*i)).map(|(_, g)| g.header().text().to_string()).collect()
            } else {
                Vec::new()
            };
            match self.get_minion_info(&row.minion_id) {
                Ok(rows) => {
                    let new_groups = SysInspectUX::build_info_tree(&rows);
                    let mut ts = TreeState::new(new_groups.len());
                    let expand_all = expanded.is_empty();
                    for (i, g) in new_groups.iter().enumerate() {
                        if expand_all || expanded.contains(&g.header().text().to_string()) {
                            ts.expand(i);
                        }
                    }
                    self.minion_traits_tree_state = Some(ts);
                    self.minion_traits_rows = rows;
                }
                Err(_) => {
                    self.minion_traits_rows = Vec::new();
                    self.minion_traits_tree_state = None;
                }
            }
        }
    }

    fn selected_popup_minion(&self) -> Option<ConsoleOnlineMinionRow> {
        match self.minions_focus {
            1 => self.filtered_online().get(self.minions_online_sel).map(|row| (*row).clone()),
            2 => self.filtered_offline().get(self.minions_offline_sel).map(|row| (*row).clone()),
            _ => None,
        }
    }

    fn console_error_alert(&mut self, op: &str, host: &str, err: SysinspectError) {
        self.error_alert_visible = true;
        self.error_alert_message = format!("{op} {host}: {err}");
        self.status_at_minions_browser();
    }

    fn do_minion_start(&mut self) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion selected".to_string();
                self.status_at_minions_browser();
                return;
            }
        };
        let host = Self::online_host(&row);
        let mid = row.minion_id.clone();
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_HOPSTART}"), "*", None, Some(&mid), None).await
            })
        }) {
            Ok(_) => self.status_at_minions_browser(),
            Err(err) => self.console_error_alert("Start", &host, err),
        }
    }

    fn do_minion_shutdown(&mut self) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion selected".to_string();
                self.status_at_minions_browser();
                return;
            }
        };
        let host = Self::online_host(&row);
        let mid = row.minion_id.clone();
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_SHUTDOWN}"), "*", None, Some(&mid), None).await
            })
        }) {
            Ok(_) => self.status_at_minions_browser(),
            Err(err) => self.console_error_alert("Shutdown", &host, err),
        }
    }

    fn do_minion_reconnect(&mut self) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion selected".to_string();
                self.status_at_minions_browser();
                return;
            }
        };
        let host = Self::online_host(&row);
        let mid = row.minion_id.clone();
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_RECONNECT}"), "*", None, Some(&mid), None).await
            })
        }) {
            Ok(_) => self.status_at_minions_browser(),
            Err(err) => self.console_error_alert("Reconnect", &host, err),
        }
    }

    fn dispatch_minion_shortcut(&mut self, e: &event::KeyEvent) -> bool {
        match e.code {
            KeyCode::Char('l') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_logs_popup();
                true
            }
            KeyCode::Char('t') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_traits_popup();
                true
            }
            KeyCode::Char('s') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.do_minion_start();
                true
            }
            KeyCode::Char('d') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.do_minion_shutdown();
                true
            }
            KeyCode::Char('f') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.do_minion_reconnect();
                true
            }
            KeyCode::Char('x') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cluster_confirm_visible = true;
                self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                self.pending_cluster_action = 1;
                true
            }
            KeyCode::Char('a') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cluster_confirm_visible = true;
                self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                self.pending_cluster_action = 2;
                true
            }
            KeyCode::Insert => {
                // TODO: do_minion_add()
                true
            }
            KeyCode::Delete => {
                self.cluster_confirm_visible = true;
                self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                self.pending_cluster_action = 3;
                true
            }
            _ => false,
        }
    }

    fn on_cluster_confirm(&mut self, e: event::KeyEvent) -> bool {
        if !self.cluster_confirm_visible {
            return false;
        }
        match e.code {
            KeyCode::Tab => {
                if self.cluster_confirm_choice == AlertResult::Default {
                    self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                } else {
                    self.cluster_confirm_choice = AlertResult::Default;
                }
            }
            KeyCode::Enter => {
                self.cluster_confirm_visible = false;
                if self.cluster_confirm_choice == AlertResult::ClusterConfirm {
                    match self.pending_cluster_action {
                        1 => self.do_cluster_shutdown(),
                        2 => self.do_cluster_reconnect(),
                        3 => self.do_minion_delete(),
                        _ => {}
                    }
                }
                self.pending_cluster_action = 0;
                self.status_at_minions_browser();
            }
            KeyCode::Esc => {
                self.cluster_confirm_visible = false;
                self.pending_cluster_action = 0;
                self.status_at_minions_browser();
            }
            _ => {}
        }
        true
    }

    fn do_cluster_shutdown(&mut self) {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_SHUTDOWN}"), "*", None, None, None).await })
        }) {
            Ok(_) => self.status_at_minions_browser(),
            Err(err) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cluster shutdown failed: {err}");
                self.status_at_minions_browser();
            }
        }
    }

    fn do_cluster_reconnect(&mut self) {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_RECONNECT}"), "*", None, None, None).await })
        }) {
            Ok(_) => self.status_at_minions_browser(),
            Err(err) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cluster reconnect failed: {err}");
                self.status_at_minions_browser();
            }
        }
    }

    fn do_minion_delete(&mut self) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion selected".to_string();
                self.status_at_minions_browser();
                return;
            }
        };
        let _host = Self::online_host(&row);
        let _mid = row.minion_id.clone();
        // TODO: call_master_console with CLUSTER_REMOVE_MINION
        self.status_at_minions_browser();
    }

    fn open_logs_popup(&mut self) {
        self.minion_logs_visible = true;
        self.minion_logs_filter = ratatui_cheese::input::InputState::new();
        self.minion_logs_filter_focus = false;
        self.status_at_minion_logs();
        if let Err(err) = self.load_selected_minion_logs() {
            self.minion_logs_visible = false;
            self.error_alert_visible = true;
            self.error_alert_message = err.to_string();
            self.status_at_minions_browser();
        }
    }

    fn open_traits_popup(&mut self) {
        self.minion_traits_visible = true;
        self.minion_traits_rows = Vec::new();
        self.minion_traits_tree_state = None;
        self.minion_traits_modified = false;
        self.minion_traits_filter = ratatui_cheese::input::InputState::new();
        self.minion_traits_filter_focus = false;
        self.status_at_minion_traits();
        self.load_selected_minion_info();
    }

    fn load_selected_minion_logs(&mut self) -> Result<(), SysinspectError> {
        let row = self.selected_popup_minion().ok_or_else(|| SysinspectError::InvalidQuery("No minion is currently selected".to_string()))?;
        let context = serde_json::json!({"stream": "merged", "lines": 200usize}).to_string();
        let host = Self::online_host(&row);
        let mid = row.minion_id.clone();
        let (source_kind, path, lines) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_LOGS}"), "*", None, Some(&mid), Some(&context)).await
            })
        })
        .and_then(|resp| match resp.payload {
            ConsolePayload::MinionLogs { snapshot } => Ok((snapshot.source_kind, snapshot.path, snapshot.lines)),
            _ => Err(SysinspectError::ProtoError("Unexpected console payload for minion logs".to_string())),
        })?;
        self.minion_logs_host = host;
        self.minion_logs_source_kind = source_kind;
        self.minion_logs_path = path;
        self.minion_logs_lines = lines;
        self.minion_logs_scroll = usize::MAX;
        self.minion_logs_online = true;
        self.minion_logs_last_fetch = Instant::now();
        Ok(())
    }

    fn on_minion_logs_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.minion_logs_visible {
            return false;
        }
        let page = self.minion_logs_viewport_rows.get().max(1);
        let total_rows = self.filtered_rendered_log_lines().len();
        let max_top = total_rows.saturating_sub(page);
        if self.minion_logs_filter_focus {
            match e.code {
                KeyCode::Esc => {
                    self.minion_logs_filter_focus = false;
                }
                KeyCode::Tab => {
                    self.minion_logs_filter_focus = false;
                }
                KeyCode::Backspace => {
                    self.minion_logs_filter.delete_before();
                }
                KeyCode::Delete => {
                    self.minion_logs_filter.delete_at();
                }
                KeyCode::Left => {
                    self.minion_logs_filter.move_left();
                }
                KeyCode::Right => {
                    self.minion_logs_filter.move_right();
                }
                KeyCode::Home => {
                    self.minion_logs_filter.home();
                }
                KeyCode::End => {
                    self.minion_logs_filter.end();
                }
                KeyCode::Char(c) => {
                    self.minion_logs_filter.insert_char(c);
                }
                _ => {}
            }
            return true;
        }
        match e.code {
            KeyCode::Esc => {
                self.minion_logs_visible = false;
                self.status_at_minions_browser();
            }
            KeyCode::Tab => {
                self.minion_logs_filter_focus = true;
            }
            KeyCode::Up => {
                if self.minion_logs_scroll == usize::MAX {
                    self.minion_logs_scroll = max_top;
                }
                self.minion_logs_scroll = self.minion_logs_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.minion_logs_scroll == usize::MAX {
                    return true;
                }
                self.minion_logs_scroll = (self.minion_logs_scroll + 1).min(max_top);
                if self.minion_logs_scroll >= max_top {
                    self.minion_logs_scroll = usize::MAX;
                }
            }
            KeyCode::PageUp => {
                if self.minion_logs_scroll == usize::MAX {
                    self.minion_logs_scroll = max_top;
                }
                self.minion_logs_scroll = self.minion_logs_scroll.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if self.minion_logs_scroll == usize::MAX {
                    return true;
                }
                self.minion_logs_scroll = (self.minion_logs_scroll + page).min(max_top);
                if self.minion_logs_scroll >= max_top {
                    self.minion_logs_scroll = usize::MAX;
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Err(err) = self.load_selected_minion_logs() {
                    self.error_alert_visible = true;
                    self.error_alert_message = err.to_string();
                }
            }
            KeyCode::Char('/') => {
                self.minion_logs_filter_focus = true;
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.minion_logs_polling = !self.minion_logs_polling;
                self.status_at_minion_logs();
            }
            _ => {}
        }
        true
    }

    fn on_tag_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.tag_visible {
            return false;
        }
        match e.code {
            KeyCode::Esc => self.tag_visible = false,
            KeyCode::Tab => {
                let prev = self.tag_focus;
                self.tag_focus = (self.tag_focus + 1) % 4;
                if prev != self.tag_focus {
                    self.tag_pos = 0;
                }
                if prev == 0
                    && self.tag_focus == 1
                    && !self.tag_key_buf.is_empty()
                    && let Some(val) = self.minion_traits_rows.iter().find(|r| r.key == self.tag_key_buf)
                {
                    self.tag_val_buf = val.value.as_str().unwrap_or_default().to_string();
                    self.tag_pos = self.tag_val_buf.len();
                }
            }
            KeyCode::BackTab => {
                self.tag_focus = (self.tag_focus + 3) % 4;
                self.tag_pos = 0;
            }
            KeyCode::Enter => match self.tag_focus {
                2 => {
                    if !self.tag_key_buf.is_empty() {
                        self.set_trait_tag();
                    }
                    self.tag_visible = false;
                }
                3 => self.tag_visible = false,
                _ => self.tag_focus = (self.tag_focus + 1) % 4,
            },
            KeyCode::Backspace => {
                let buf = if self.tag_focus == 0 { &mut self.tag_key_buf } else { &mut self.tag_val_buf };
                self.tag_pos = Self::snap_char_boundary(buf, self.tag_pos);
                if self.tag_pos > 0 {
                    self.tag_pos = Self::prev_char_boundary(buf, self.tag_pos);
                    buf.remove(self.tag_pos);
                }
            }
            KeyCode::Left => self.shift_tag_pos(-1),
            KeyCode::Right => self.shift_tag_pos(1),
            KeyCode::Home => self.tag_pos = 0,
            KeyCode::End => {
                let buf = if self.tag_focus == 0 { &self.tag_key_buf } else { &self.tag_val_buf };
                self.tag_pos = buf.len();
            }
            KeyCode::Delete => {
                let buf = if self.tag_focus == 0 { &mut self.tag_key_buf } else { &mut self.tag_val_buf };
                if self.tag_pos < buf.len() && buf.is_char_boundary(self.tag_pos) {
                    buf.remove(self.tag_pos);
                }
            }
            KeyCode::Char(c) => {
                let buf = if self.tag_focus == 0 { &mut self.tag_key_buf } else { &mut self.tag_val_buf };
                self.tag_pos = Self::snap_char_boundary(buf, self.tag_pos);
                buf.insert(self.tag_pos, c);
                self.tag_pos += c.len_utf8();
            }
            _ => {}
        }
        true
    }

    fn shift_tag_pos(&mut self, dir: i8) {
        let buf = if self.tag_focus == 0 { &self.tag_key_buf } else { &self.tag_val_buf };
        if dir < 0 {
            self.tag_pos = Self::prev_char_boundary(buf, self.tag_pos);
        } else {
            self.tag_pos = Self::next_char_boundary(buf, self.tag_pos).unwrap_or(buf.len());
        }
    }

    fn prev_char_boundary(s: &str, pos: usize) -> usize {
        let mut p = pos.saturating_sub(1);
        while p > 0 && !s.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn next_char_boundary(s: &str, pos: usize) -> Option<usize> {
        let mut p = (pos + 1).min(s.len());
        while p < s.len() && !s.is_char_boundary(p) {
            p += 1;
        }
        if p <= s.len() && s.is_char_boundary(p) { Some(p) } else { None }
    }

    fn snap_char_boundary(s: &str, pos: usize) -> usize {
        Self::next_char_boundary(s, pos).unwrap_or_else(|| Self::prev_char_boundary(s, pos))
    }

    fn set_trait_tag(&mut self) {
        let filtered = self.filtered_online();
        let mid = match filtered.get(self.minions_online_sel) {
            Some(r) => r.minion_id.clone(),
            None => return,
        };
        let key = self.tag_key_buf.clone();
        let val = self.tag_val_buf.clone();
        let op = if val.is_empty() { "unset" } else { "set" };
        let traits = serde_json::json!({&key: &val});
        let context = serde_json::json!({"op": op, "traits": traits}).to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                if let Err(err) =
                    call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_TRAITS_UPDATE}"), "*", None, Some(&mid), Some(&context)).await
                {
                    log::error!("Failed to set trait: {err}");
                }
            })
        });
        let expanded: Vec<String> = if let Some(ref ts) = self.minion_traits_tree_state {
            let old_groups = SysInspectUX::build_info_tree(&self.minion_traits_rows);
            old_groups.iter().enumerate().filter(|(i, _)| ts.is_expanded(*i)).map(|(_, g)| g.header().text().to_string()).collect()
        } else {
            Vec::new()
        };
        if val.is_empty() {
            self.minion_traits_rows.retain(|r| r.key != key);
        } else if let Some(existing) = self.minion_traits_rows.iter_mut().find(|r| r.key == key) {
            existing.value = serde_json::Value::String(val);
        } else {
            self.minion_traits_rows.push(ConsoleMinionInfoRow {
                key,
                value: serde_json::Value::String(val),
                source: libsysinspect::traits::TraitSource::Static,
            });
        }
        let new_groups = SysInspectUX::build_info_tree(&self.minion_traits_rows);
        let mut ts = TreeState::new(new_groups.len());
        for (i, g) in new_groups.iter().enumerate() {
            if expanded.contains(&g.header().text().to_string()) {
                ts.expand(i);
            }
        }
        self.minion_traits_tree_state = Some(ts);
        self.minion_traits_modified = true;
    }

    /// Update cycles on up/down keystrokes
    fn on_update_cycles(&mut self, down: bool) {
        match self.get_cycles() {
            Ok(cycles) => {
                self.cycles_buf = cycles;
                self.minions_buf = Vec::new();
                self.events_buf = Vec::new();
                self.event_data = IndexMap::new();

                if down {
                    if self.selected_cycle < self.cycles_buf.len().saturating_sub(1) {
                        self.selected_cycle += 1;
                    }
                } else if self.selected_cycle > 0 {
                    self.selected_cycle -= 1;
                }
            }
            Err(err) => {
                self.error_alert_visible = true;
                self.error_alert_message = err.to_string();
            }
        }
    }

    fn on_key(&mut self, e: event::KeyEvent) {
        // Error alert is modal — always checked first
        if self.on_error_alert(e) {
            return;
        }

        // Information alert is modal
        if self.on_info_alert(e) {
            return;
        }

        // Exit alert takes priority over setup wizard
        if self.on_exit_alert(e) {
            return;
        }

        // Setup wizard is modal
        if self.setup_wizard.visible {
            self.setup_wizard.handle_key(e);
            if self.setup_wizard.quit_requested {
                self.setup_wizard.quit_requested = false;
                self.exit_alert_visible = true;
                self.exit_alert_choice = AlertResult::Default;
            }
            return;
        }
        if self.on_error_alert(e) {
            return;
        }

        if self.dsl_browser.visible {
            self.dsl_browser.handle_key(e.code);
            if !self.dsl_browser.visible {
                self.restore_status();
            }
            if self.dsl_browser.call_requested {
                self.dsl_browser.call_requested = false;
                if let Some(query) = self.dsl_browser.take_query() {
                    let minion_query = if self.dsl_browser.query.is_empty() || self.dsl_browser.query == "*" {
                        "*".to_string()
                    } else {
                        self.dsl_browser.query.clone()
                    };
                    let ctx = self.dsl_browser.build_context_json();
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            if let Err(err) = call_master_console(&self.cfg, &query, &minion_query, None, None, ctx.as_ref()).await {
                                log::error!("Call failed: {err}");
                            }
                        })
                    });
                } else {
                    let model = self.dsl_browser.models.items.get(self.dsl_browser.models.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
                    let _target =
                        self.dsl_browser.targets.items.get(self.dsl_browser.targets.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
                    let missing_keys = std::mem::take(&mut self.dsl_browser.error_required_key);
                    if !missing_keys.is_empty() {
                        self.error_alert_visible = true;
                        let list: String = missing_keys.iter().map(|k| format!("  - {k}")).collect::<Vec<_>>().join("\n");
                        self.error_alert_message = format!("Required context fields are missing:\n{list}");
                    } else {
                        let missing = if model == "(select)" || model == "(no models found)" { "Model" } else { "Target" };
                        self.error_alert_visible = true;
                        self.error_alert_message = format!("Select {missing} first!");
                    }
                }
            }
            return;
        }

        if self.on_help_popup(e) {
            return;
        }

        if self.on_purge_alert(e) {
            return;
        }

        if self.on_cluster_confirm(e) {
            return;
        }

        if self.on_error_alert(e) {
            return;
        }

        if self.on_tag_popup(e) {
            return;
        }

        if self.on_minions_popup(e) {
            return;
        }

        match e.code {
            KeyCode::PageUp => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        self.selected_cycle = self.selected_cycle.saturating_sub(self.size.get().table_cycles);
                    }
                    ActiveBox::Minions => {
                        self.selected_minion = self.selected_minion.saturating_sub(self.size.get().table_minions);
                    }
                    ActiveBox::Events => {
                        self.selected_event = self.selected_event.saturating_sub(self.size.get().table_events);
                    }
                    ActiveBox::Info => {
                        self.actdt_info_offset = self.actdt_info_offset.saturating_sub(self.size.get().table_info);
                    }
                };
            }
            KeyCode::PageDown => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        self.selected_cycle = (self.selected_cycle + self.size.get().table_cycles).min(self.cycles_buf.len().saturating_sub(1));
                    }
                    ActiveBox::Minions => {
                        if !self.li_events.is_empty() {
                            self.selected_minion =
                                (self.selected_minion + self.size.get().table_minions).min(self.li_minions.len().saturating_sub(1));
                        }
                    }
                    ActiveBox::Events => {
                        if !self.li_events.is_empty() {
                            self.selected_event = (self.selected_event + self.size.get().table_events).min(self.li_events.len().saturating_sub(1));
                        }
                    }
                    ActiveBox::Info => {
                        self.actdt_info_offset =
                            (self.actdt_info_offset + self.size.get().table_info).min(self.info_rows.borrow().len().saturating_sub(1));
                    }
                };
            }
            KeyCode::Up => {
                match self.active_box {
                    ActiveBox::Cycles => self.on_update_cycles(false),
                    ActiveBox::Minions => {
                        if self.selected_minion > 0 {
                            self.selected_minion -= 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event > 0 {
                            self.selected_event -= 1;
                        }
                        self.event_data = self.get_selected_event().unwrap().event().flatten();
                    }
                    ActiveBox::Info => {
                        if self.actdt_info_offset > 0 {
                            self.actdt_info_offset -= 1;
                        }
                    }
                };
            }
            KeyCode::Down => {
                match self.active_box {
                    ActiveBox::Cycles => self.on_update_cycles(true),
                    ActiveBox::Minions => {
                        if self.selected_minion < self.li_minions.len().saturating_sub(1) {
                            self.selected_minion += 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event < self.li_events.len().saturating_sub(1) {
                            self.selected_event += 1;
                        }
                        self.event_data = self.get_selected_event().unwrap().event().flatten();
                    }
                    ActiveBox::Info => {
                        let total = self.info_rows.borrow().len();
                        if self.actdt_info_offset < total.saturating_sub(1) {
                            self.actdt_info_offset += 1;
                        }
                    }
                };
            }
            KeyCode::Right => {
                self.shift_next();
            }
            KeyCode::Left => {
                self.shift_prev();
            }
            KeyCode::Enter => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        if let Ok(cycles) = self.get_cycles() {
                            self.cycles_buf = cycles;
                            self.minions_buf = Vec::new();
                            self.events_buf = Vec::new();
                            self.event_data = IndexMap::new();

                            if !self.cycles_buf.is_empty() {
                                match self.get_minions(self.get_selected_cycle().event().sid()) {
                                    Ok(minions) => {
                                        self.li_minions = minions;
                                        self.selected_minion = 0;
                                        self.selected_event = 0;
                                    }
                                    Err(err) => {
                                        self.error_alert_visible = true;
                                        self.error_alert_message = err.to_string();
                                    }
                                }
                            }
                            self.shift_next();
                        } else {
                            self.error_alert_visible = true;
                        }
                    }
                    ActiveBox::Minions => {
                        // Reset if no cycles
                        if !self.li_minions.is_empty() {
                            if let Some(mli) = self.get_selected_minion() {
                                match self.get_events(self.get_selected_cycle().event().sid(), mli.event().id()) {
                                    Ok(events) => {
                                        self.li_events = events;
                                        self.selected_event = 0;
                                        self.event_data = IndexMap::new();
                                    }
                                    Err(err) => {
                                        self.error_alert_visible = true;
                                        self.error_alert_message = err.to_string();
                                    }
                                }
                            }
                            self.selected_event = 0;
                        }
                        self.shift_next();
                    }
                    ActiveBox::Events if !self.li_events.is_empty() && self.get_selected_event().is_some() => {
                        self.status_at_action_data();
                        self.active_box = ActiveBox::Info;
                        self.event_data = self.get_selected_event().unwrap().event().flatten();
                    }
                    _ => {}
                };
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.exit_alert_visible = true;
                self.exit_alert_choice = AlertResult::Quit;
            }
            KeyCode::Char('p') => {
                self.purge_alert_visible = true;
                self.purge_alert_choice = AlertResult::Default;
            }
            KeyCode::Char('h') => {
                self.help_popup_visible = true;
            }
            KeyCode::Char('c') => match self.get_models() {
                Ok((rows, failures)) => {
                    self.dsl_browser.load_models(rows, failures);
                    self.status_at_query_composer();
                    if let Ok(minions) = self.fetch_minions() {
                        let names: Vec<String> =
                            minions.iter().map(|r| if !r.fqdn.is_empty() { r.fqdn.clone() } else { r.hostname.clone() }).collect();
                        self.dsl_browser.set_minions(names);
                    }
                }
                Err(err) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Failed to load models: {err}");
                }
            },
            KeyCode::Char('o') if !e.modifiers.contains(KeyModifiers::CONTROL) => match self.fetch_minions() {
                Ok(rows) if rows.is_empty() => {
                    self.error_alert_visible = true;
                    self.error_alert_message = "No minions registered yet".to_string();
                }
                Ok(rows) => {
                    self.minions_rows = rows;
                    self.minions_visible = true;
                    self.minions_focus = 1;
                    self.minions_online_sel = 0;
                    self.minions_offline_sel = 0;
                    self.minions_filter_input = ratatui_cheese::input::InputState::new();
                    self.minion_traits_visible = false;
                    self.minion_traits_rows = Vec::new();
                    self.minion_traits_tree_state = None;
                    self.minion_traits_modified = false;
                    self.status_at_minions_browser();
                }
                Err(err) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Failed to get online minions: {err}");
                }
            },

            KeyCode::BackTab if self.active_box == ActiveBox::Info => {
                self.status_at_action_results();
                self.active_box = ActiveBox::Events;
            }

            KeyCode::Tab if self.active_box == ActiveBox::Events => {
                self.status_at_action_data();
                self.active_box = ActiveBox::Info;
                self.event_data = self.get_selected_event().unwrap().event().flatten();
            }

            _ => {}
        }
    }

    /// Get selected cycle from the menu
    fn get_selected_cycle(&self) -> &CycleListItem {
        &self.cycles_buf[self.selected_cycle]
    }

    /// Get selected minion from the menu
    fn get_selected_minion(&self) -> Option<&MinionListItem> {
        if self.li_minions.is_empty() || self.li_minions.len() <= self.selected_minion {
            return None;
        }

        Some(&self.li_minions[self.selected_minion])
    }

    /// Get selected event from the menu
    fn get_selected_event(&self) -> Option<&EventListItem> {
        if self.li_events.is_empty() || self.li_events.len() <= self.selected_event {
            return None;
        }

        Some(&self.li_events[self.selected_event])
    }

    fn exit(&mut self) {
        self.exit = true;
    }
    /// Returns a vector of cycle names.
    pub fn get_cycles(&self) -> Result<Vec<CycleListItem>, SysinspectError> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let rq = match c_ipc.lock().await.query("", "", "", QUERY_CYCLES).await {
                        Ok(rq) => rq,
                        Err(err) => {
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {err}")));
                        }
                    };

                    let mut cycles: Vec<CycleListItem> = rq
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            let s = EventSession::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec()).unwrap();
                            CycleListItem::new(s.get_ts_mask(None).as_str(), s)
                        })
                        .collect();
                    cycles.sort_by_key(|ts| std::cmp::Reverse(ts.event().get_ts_unix()));
                    Ok(cycles)
                })
            });
        }
        Ok(vec![])
    }

    /// Returns a vector of minion names (random IDs).
    /// Params:
    /// - `sid`: Session ID (cycle)
    pub fn get_minions(&self, sid: &str) -> Result<Vec<MinionListItem>, SysinspectError> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let r = match c_ipc.lock().await.query("", "", sid, QUERY_MINIONS).await {
                        Ok(r) => r,
                        Err(err) => {
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {err}")));
                        }
                    };
                    let minions: Vec<MinionListItem> = r
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            let mut s = EventMinion::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec()).unwrap();
                            s.set_cid(rec.tree);
                            MinionListItem::new(s)
                        })
                        .collect();
                    Ok(minions)
                })
            });
        }

        Ok(vec![])
    }

    /// Returns a vector of events for a particular minion.
    /// Params:
    /// - `sid`: Session ID (cycle)
    /// - `mid`: Minion ID
    pub fn get_events(&self, sid: &str, mid: &str) -> Result<Vec<EventListItem>, SysinspectError> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let r = match c_ipc.lock().await.query(mid, "", sid, QUERY_EVENTS).await {
                        Ok(r) => r,
                        Err(err) => {
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {err}")));
                        }
                    };
                    let events: Vec<EventListItem> = r
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            EventListItem::new(EventData::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec()).unwrap())
                        })
                        .collect();
                    Ok(events)
                })
            });
        }
        Ok(vec![])
    }

    fn purge_database(&mut self) -> Result<(), SysinspectError> {
        let out = if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    match c_ipc.lock().await.query("", "", "", QUERY_CMD_PURGE_ALL).await {
                        Ok(_) => Ok(()),
                        Err(err) => Err(SysinspectError::ProtoError(format!("Error purging data: {err}"))),
                    }
                })
            })
        } else {
            Ok(())
        };

        // Reset the UI
        if out.is_ok() {
            self.active_box = ActiveBox::Cycles;
            self.selected_cycle = 0;
            self.selected_minion = 0;
            self.selected_event = 0;
            self.cycles_buf = Vec::new();
            self.minions_buf = Vec::new();
            self.events_buf = Vec::new();
            self.event_data = IndexMap::new();
            self.li_minions = Vec::new();
            self.li_events = Vec::new();

            self.status_at_cycles();
            self.on_update_cycles(false);
        }

        out
    }

    /// Query the master console for currently online minions.
    pub fn fetch_minions(&self) -> Result<Vec<ConsoleOnlineMinionRow>, SysinspectError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}"), "*", None, None, None).await.map(|resp| {
                    match resp.payload {
                        ConsolePayload::OnlineMinions { rows } => rows,
                        _ => Vec::new(),
                    }
                })
            })
        })
    }

    /// Query the master console for available models.
    pub fn get_models(&self) -> Result<(Vec<ConsoleModelRow>, Vec<String>), SysinspectError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MODELS}"), "*", None, None, None).await.map(|resp| {
                    match resp.payload {
                        ConsolePayload::Models { rows, failures } => (rows, failures),
                        _ => (Vec::new(), Vec::new()),
                    }
                })
            })
        })
    }

    /// Query the master console for detailed minion info.
    pub fn get_minion_info(&self, mid: &str) -> Result<Vec<ConsoleMinionInfoRow>, SysinspectError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_INFO}"), "*", None, Some(mid), None).await.map(|resp| {
                    match resp.payload {
                        ConsolePayload::MinionInfo { rows } => rows,
                        _ => Vec::new(),
                    }
                })
            })
        })
    }

    /// Count the vertical space for the alert display, plus three empty lines
    fn get_text_lines(s: &str) -> u16 {
        s.matches('\n').count() as u16 + 3
    }

    /// Get the maximum width of the lines
    fn get_max_width_lines(s: &str) -> u16 {
        s.lines().map(|l| UnicodeWidthStr::width(l) as u16).max().unwrap_or_default()
    }
}
