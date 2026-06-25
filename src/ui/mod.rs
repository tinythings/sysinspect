use crate::{call_master_console, ui::elements::DbListItem};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    execute,
};
use elements::{ActiveBox, AlertResult, CycleListItem, EventListItem, MinionListItem};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libeventreg::{
    QUERY_CMD_PURGE_ALL, QUERY_CYCLES, QUERY_EVENTS, QUERY_MINIONS,
    ipcc::DbIPCClient,
    kvdb::{EventData, EventMinion, EventSession},
};
use libmodcore::modinit::ModInterface;
use libmodpak::{SysInspectModPak, mpk::ModPakMetadata};
use libsysinspect::{
    cfg::mmconf::{ConsoleConfig, MasterConfig, MinionConfig},
    console::{ConsoleMinionInfoRow, ConsoleModelRow, ConsoleModuleRow, ConsoleOnlineMinionRow, ConsolePayload},
    mdescr::catalog::ModelCatalog,
    traits::os_display_name,
};
use libsysproto::query::{
    SCHEME_COMMAND,
    commands::{
        CLUSTER_CONFIG_RELOAD, CLUSTER_HOPSTART, CLUSTER_LIBRARY_INDEX, CLUSTER_MARK_UPGRADE_REQUIRED, CLUSTER_MASTER_LOGS, CLUSTER_MINION_HOPSTART,
        CLUSTER_MINION_INFO, CLUSTER_MINION_LOGS, CLUSTER_MINION_PROCESS_SIGNAL, CLUSTER_MINION_RECONNECT, CLUSTER_MINION_SHUTDOWN,
        CLUSTER_MINION_TOP, CLUSTER_MODELS, CLUSTER_MODULE_INDEX, CLUSTER_ONLINE_MINIONS, CLUSTER_PROFILE, CLUSTER_RECONNECT, CLUSTER_REMOVE_MINION,
        CLUSTER_SHUTDOWN, CLUSTER_SYNC, CLUSTER_TRAITS_UPDATE, CLUSTER_UPGRADE_MINIONS, CLUSTER_UPGRADE_STATUS,
    },
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Row},
};
use ratatui_cheese::tree::TreeState;
use ratatui_glamour::widgets::spinner;
use std::{
    cell::{Cell, RefCell},
    io::{self},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use unicode_width::UnicodeWidthStr;

mod alert;
mod dslbrowser;
mod elements;
mod filepicker;
mod macts;
mod minreg;
mod online;
mod palette;
mod platforms;
mod profiles;
#[cfg(test)]
mod profiles_ut;
mod rawlogs;
mod repomanager;
mod setup;
mod statusbar;
mod systop;
mod title;
mod traitsview;
mod traittag;
mod typecolors;
mod wgt;

use alert::DialogFormFocus;

pub async fn run(cfg: MasterConfig, config_found: bool) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), crossterm::event::EnableMouseCapture);
    let result = tokio_run(cfg, config_found, &mut terminal).await;
    let _ = execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();

    result
}

async fn tokio_run(cfg: MasterConfig, config_found: bool, term: &mut DefaultTerminal) -> io::Result<()> {
    match SysInspectUX::new(cfg.clone()).await {
        Ok(app) => app.run_connected(term),
        Err(_) if config_found => {
            let mut app = SysInspectUX { cfg, offline: true, ..Default::default() };
            let _ = app.load_console_tool_preferences();
            if app.find_sysmaster_binary().is_some() {
                app.master_confirm_visible = true;
                app.master_confirm_choice = AlertResult::Default;
                app.master_confirm_action = 1;
                app.run_offline_loop(term)
            } else {
                app.setup_wizard = setup::MasterSetupWizard::from_config(&app.cfg);
                app.setup_wizard.visible = true;
                app.run_setup_loop(term, &mut None)
            }
        }
        Err(_) => {
            let mut app = SysInspectUX { cfg, setup_wizard: setup::MasterSetupWizard { visible: true, ..Default::default() }, ..Default::default() };
            let _ = app.load_console_tool_preferences();
            app.run_setup_loop(term, &mut None)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UISizes {
    pub table_cycles: usize,
    pub table_minions: usize,
    pub table_events: usize,
    pub table_info: usize,
}

#[derive(Debug)]
pub struct DeleteProgressState {
    pub visible: bool,
    pub message: String,
    pub spinner: spinner::Model,
    pub last_tick: Instant,
}

impl Default for DeleteProgressState {
    fn default() -> Self {
        let mut spinner_model = spinner::Model::new();
        spinner_model.spinner = spinner::Spinner::mini_dot();
        spinner_model.style = Style::default().fg(palette::PROCESSING_PEAK);
        Self { visible: false, message: String::new(), spinner: spinner_model, last_tick: Instant::now() }
    }
}

#[derive(Debug)]
pub struct ClusterUpgradeProgressState {
    pub visible: bool,
    pub message: String,
    pub spinner: spinner::Model,
    pub last_tick: Instant,
}

impl Default for ClusterUpgradeProgressState {
    fn default() -> Self {
        let mut spinner_model = spinner::Model::new();
        spinner_model.spinner = spinner::Spinner::mini_dot();
        spinner_model.style = Style::default().fg(palette::WARNING_PEAK);
        Self { visible: false, message: String::new(), spinner: spinner_model, last_tick: Instant::now() }
    }
}

type ClusterUpgradeTaskResult = Result<(usize, usize, usize, usize, usize, Vec<String>), String>;

type ClusterStartTaskResult = Result<(usize, Vec<String>), String>;

type SetupTaskResult = Result<setup::SetupCompletion, String>;

#[derive(Debug)]
pub struct ClusterStartProgressState {
    pub visible: bool,
    pub message: String,
    pub spinner: spinner::Model,
    pub last_tick: Instant,
}

impl Default for ClusterStartProgressState {
    fn default() -> Self {
        let mut spinner_model = spinner::Model::new();
        spinner_model.spinner = spinner::Spinner::mini_dot();
        spinner_model.style = Style::default().fg(palette::PROCESSING_PEAK);
        Self { visible: false, message: String::new(), spinner: spinner_model, last_tick: Instant::now() }
    }
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
    pub info_alert_title: String,
    pub info_alert_styled: Option<Text<'static>>,

    /// Exit alert
    pub exit_alert_visible: bool,
    pub exit_alert_choice: AlertResult,

    // Help popup
    pub help_popup_visible: bool,
    pub help_popup_scroll: Cell<usize>,

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

    pub systop: systop::SystemTopState,

    // Master logs popup
    pub master_logs_visible: bool,
    pub master_logs_tab: usize,
    pub master_logs_sections: Vec<rawlogs::LogSection>,
    pub master_logs_filter: ratatui_cheese::input::InputState,
    pub master_logs_filter_focus: bool,
    pub master_logs_polling: bool,
    pub master_logs_last_fetch: Instant,
    pub master_logs_viewport_rows: Cell<usize>,
    pub master_menu_visible: bool,
    pub master_menu_sel: usize,
    pub master_confirm_visible: bool,
    pub master_confirm_choice: AlertResult,
    pub master_confirm_action: u8, // 0=none, 1=start, 2=restart, 3=stop

    // Online minions action menu
    pub minions_menu_visible: bool,
    pub minions_menu_sel: usize,

    // Cluster-wide operation confirmation
    pub cluster_confirm_visible: bool,
    pub cluster_confirm_choice: AlertResult,
    pub pending_cluster_action: u8, // 0=none, 1=shutdown all, 2=reconnect all, 3=delete minion
    pub delete_force_remove: bool,  // checkbox: also remove from host over SSH
    cluster_confirm_form_focus: DialogFormFocus,
    pub(crate) popup_button_rects: Cell<Option<alert::PopupButtonRects>>,

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

    // File picker
    pub file_picker: filepicker::FilePicker,

    // Artefacts manager
    pub repo_manager: repomanager::RepoManager,

    // Connection state
    pub offline: bool,
    pub last_reconnect_attempt: Instant,

    // Minion registration
    pub registration_form: minreg::RegistrationForm,
    pub registration_progress: Arc<std::sync::Mutex<minreg::RegistrationProgress>>,
    pub registration_task: Option<tokio::task::JoinHandle<()>>,
    pub setup_progress: Arc<std::sync::Mutex<setup::SetupProgress>>,
    pub setup_task: Option<tokio::task::JoinHandle<SetupTaskResult>>,
    pub delete_progress: DeleteProgressState,
    pub delete_task: Option<tokio::task::JoinHandle<Result<String, String>>>,
    pub delete_success_message: String,
    pub delete_success_styled: Option<Text<'static>>,
    pub cluster_upgrade_progress: ClusterUpgradeProgressState,
    pub cluster_upgrade_task: Option<tokio::task::JoinHandle<ClusterUpgradeTaskResult>>,
    pub cluster_upgrade_required_count: usize,
    pub cluster_upgrade_unreachable_count: usize,
    pub cluster_upgrade_pending_count: usize,
    pub cluster_upgrade_check_message: Option<String>,
    pub cluster_start_progress: ClusterStartProgressState,
    pub cluster_start_task: Option<tokio::task::JoinHandle<ClusterStartTaskResult>>,

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
            info_alert_title: String::new(),
            info_alert_styled: None,
            help_popup_visible: false,
            help_popup_scroll: Cell::new(0),

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

            systop: systop::SystemTopState::default(),

            master_logs_visible: false,
            master_logs_tab: 0,
            master_logs_sections: Vec::new(),
            master_logs_filter: ratatui_cheese::input::InputState::new(),
            master_logs_filter_focus: false,
            master_logs_polling: true,
            master_logs_last_fetch: Instant::now(),
            master_logs_viewport_rows: Cell::new(0),
            master_menu_visible: false,
            master_menu_sel: 0,
            master_confirm_visible: false,
            master_confirm_choice: AlertResult::default(),
            master_confirm_action: 0,

            minions_menu_visible: false,
            minions_menu_sel: 0,

            cluster_confirm_visible: false,
            cluster_confirm_choice: AlertResult::default(),
            pending_cluster_action: 0,
            delete_force_remove: false,
            cluster_confirm_form_focus: DialogFormFocus::LeftButton,
            popup_button_rects: Cell::new(None),

            tag_visible: false,
            tag_key_buf: String::new(),
            tag_val_buf: String::new(),
            tag_focus: 0,
            tag_pos: 0,

            evtipc: None,
            dsl_browser: dslbrowser::DslBrowser::new(),
            cfg: MasterConfig::default(),
            setup_wizard: setup::MasterSetupWizard::default(),
            file_picker: filepicker::FilePicker::default(),
            repo_manager: repomanager::RepoManager::default(),
            offline: false,
            last_reconnect_attempt: Instant::now(),

            registration_form: minreg::RegistrationForm::default(),
            registration_progress: Arc::new(std::sync::Mutex::new(minreg::RegistrationProgress::placeholder())),
            registration_task: None,
            setup_progress: Arc::new(std::sync::Mutex::new(setup::SetupProgress::hidden())),
            setup_task: None,
            delete_progress: DeleteProgressState::default(),
            delete_task: None,
            delete_success_message: String::new(),
            delete_success_styled: None,
            cluster_upgrade_progress: ClusterUpgradeProgressState::default(),
            cluster_upgrade_task: None,
            cluster_upgrade_required_count: 0,
            cluster_upgrade_unreachable_count: 0,
            cluster_upgrade_pending_count: 0,
            cluster_upgrade_check_message: None,
            cluster_start_progress: ClusterStartProgressState::default(),
            cluster_start_task: None,

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
        ux.load_console_tool_preferences()?;

        Ok(ux)
    }

    fn load_console_tool_preferences(&mut self) -> Result<(), SysinspectError> {
        let console_cfg = ConsoleConfig::new(self.cfg.config_path())?;
        let systop = console_cfg.system_top();
        self.systop.set_persisted_preferences(systop.sort, systop.graph());
        Ok(())
    }

    pub fn run_loop(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.cycles_buf = self.get_cycles().unwrap_or_default();
        if !self.cycles_buf.is_empty() {
            let sid = self.get_selected_cycle().event().sid().to_string();
            if let Ok(minions) = self.get_minions(&sid) {
                self.li_minions = minions;
                self.refresh_events_for_selected_minion();
            }
        }
        self.refresh_cluster_upgrade_status();
        self.run_normal_loop(term)
    }

    fn run_connected(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.cycles_buf = self.get_cycles().unwrap_or_default();
        if !self.cycles_buf.is_empty() {
            let sid = self.get_selected_cycle().event().sid().to_string();
            if let Ok(minions) = self.get_minions(&sid) {
                self.li_minions = minions;
                self.refresh_events_for_selected_minion();
            }
        }
        self.refresh_cluster_upgrade_status();
        self.run_normal_loop(term)
    }

    pub fn run_setup_loop(mut self, term: &mut DefaultTerminal, exit_msg: &mut Option<String>) -> io::Result<()> {
        self.last_reconnect_attempt = Instant::now();
        self.no_focus = true;

        while !self.exit {
            term.draw(|frame| self.draw(frame))?;
            if self.setup_wizard.ok_pressed {
                if self.setup_wizard.sysmaster_path.value().is_empty() {
                    self.error_alert_visible = true;
                    self.error_alert_message = "Sys Master binary must be selected.".to_string();
                    self.setup_wizard.ok_pressed = false;
                    self.setup_wizard.focus = setup::SetupFocus::SysMasterPath;
                } else {
                    self.setup_wizard.ok_pressed = false;
                    self.setup_wizard.visible = false;
                    let request = self.setup_wizard.to_request();
                    let progress = self.setup_progress.clone();
                    if let Ok(mut state) = progress.lock() {
                        *state = setup::SetupProgress::new();
                    }
                    self.setup_task = Some(tokio::task::spawn_blocking(move || request.run_with_progress(progress)));
                }
            }
            if self.setup_task.as_ref().is_some_and(|task| task.is_finished())
                && let Some(task) = self.setup_task.take()
            {
                let result = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task));
                if let Ok(mut progress) = self.setup_progress.lock() {
                    *progress = setup::SetupProgress::hidden();
                }
                match result {
                    Ok(Ok(done)) => {
                        self.pending_exit = true;
                        self.info_alert_visible = true;
                        self.info_alert_title = "Master Installation Complete".to_string();
                        self.info_alert_styled = Some(done.info_styled);
                        self.info_alert_message = done.info_message;
                    }
                    Ok(Err(err)) => {
                        self.error_alert_visible = true;
                        self.error_alert_message = err;
                        self.setup_wizard.visible = true;
                    }
                    Err(err) => {
                        self.error_alert_visible = true;
                        self.error_alert_message = format!("Setup task failed: {err}");
                        self.setup_wizard.visible = true;
                    }
                }
            }
            if self.setup_task.is_none() && !self.error_alert_visible && !self.setup_wizard.visible && self.try_reconnect_silent().is_ok() {
                return self.run_normal_loop(term);
            }
            // Periodic silent reconnect in setup mode
            if self.setup_task.is_none()
                && !self.setup_wizard.ok_pressed
                && self.last_reconnect_attempt.elapsed() >= Duration::from_secs(5)
                && self.evtipc.is_some()
            {
                self.last_reconnect_attempt = Instant::now();
                if self.try_reconnect_silent().is_ok() {
                    return self.run_normal_loop(term);
                }
            }
            // Launch file picker for sysmaster selection
            if self.setup_wizard.launch_file_picker {
                self.setup_wizard.launch_file_picker = false;
                let start_dir = std::path::Path::new(&self.setup_wizard.sysmaster_path.value())
                    .parent()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                self.file_picker.open(&start_dir, filepicker::PickerMode::FilePicker);
            }
            // Launch dir picker for custom destination
            if self.setup_wizard.launch_dir_picker {
                self.setup_wizard.launch_dir_picker = false;
                let start_dir = std::path::PathBuf::from(self.setup_wizard.custom_destination.value());
                self.file_picker.open(&start_dir, filepicker::PickerMode::DirectoryPicker);
            }
            // File/dir picker result
            if let Some(path) = self.file_picker.selected.take() {
                match self.file_picker.mode {
                    filepicker::PickerMode::DirectoryPicker => {
                        self.setup_wizard.custom_destination.set_value(path.to_string_lossy().to_string());
                    }
                    _ => {
                        self.setup_wizard.sysmaster_path.set_value(path.to_string_lossy().to_string());
                    }
                }
            }
            // File picker status bar overrides everything
            if self.file_picker.visible {
                self.status_text = self.file_picker.status_line();
            }
            if self.repo_manager.visible {
                self.status_at_repo_manager();
            }
            // Status bar for sysmaster path focus
            if self.setup_wizard.focus == setup::SetupFocus::SysMasterPath {
                self.status_text = Line::from(vec![
                    Span::styled(" Enter ", Style::default().fg(palette::FG)),
                    Span::styled("to browse for sysmaster binary", Style::default().fg(palette::FAINT)),
                ]);
            }
            // Status bar for custom destination focus
            if self.setup_wizard.focus == setup::SetupFocus::CustomDest {
                self.status_text = Line::from(vec![
                    Span::styled(" Enter ", Style::default().fg(palette::FG)),
                    Span::styled("to browse for directory", Style::default().fg(palette::FAINT)),
                ]);
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
            if self.file_picker.visible {
                self.status_text = self.file_picker.status_line();
            }
            if self.repo_manager.visible {
                self.status_at_repo_manager();
            }
            if let Some(ref msg) = self.cluster_upgrade_check_message
                && self.cluster_upgrade_required_count == 0
            {
                let mut spans: Vec<Span> = self.status_text.clone().spans.to_vec();
                spans.push(Span::styled(format!("  {}", msg), Style::default().fg(palette::MUTED)));
                self.status_text = Line::from(spans);
            }
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
            if self.file_picker.visible {
                self.status_text = self.file_picker.status_line();
            }
            if self.repo_manager.visible {
                self.status_at_repo_manager();
            }
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
        let poll_dur = if self.setup_progress.lock().unwrap().visible { Duration::from_millis(50) } else { Duration::from_secs(1) };
        if event::poll(poll_dur)?
            && let Event::Key(e) = event::read()?
            && e.kind == KeyEventKind::Press
        {
            if self.setup_wizard.visible
                && !self.error_alert_visible
                && !self.exit_alert_visible
                && !self.info_alert_visible
                && !self.file_picker.visible
            {
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

        let badge_text = if self.cluster_upgrade_required_count > 0 {
            Some(if self.cluster_upgrade_unreachable_count > 0 { " ⚠️ Cluster upgrade incomplete " } else { " 🚨 Cluster upgrade required " })
        } else {
            None
        };

        if self.offline {
            let offline_w: u16 = 14;
            let (main_status, badge_area, offline_area) = if let Some(text) = badge_text {
                let badge_w = UnicodeWidthStr::width(text) as u16;
                let [main_status, badge_area, offline_area]: [Rect; 3] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(0), Constraint::Length(badge_w), Constraint::Length(offline_w)].as_ref())
                    .split(status_area)
                    .as_ref()
                    .try_into()
                    .unwrap();
                (main_status, Some((badge_area, text)), offline_area)
            } else {
                let [main_status, offline_area]: [Rect; 2] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(0), Constraint::Length(offline_w)].as_ref())
                    .split(status_area)
                    .as_ref()
                    .try_into()
                    .unwrap();
                (main_status, None, offline_area)
            };

            let status_paragraph = Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
            frame.render_widget(status_paragraph, main_status);
            if let Some((badge_area, text)) = badge_area {
                let badge_style = if self.cluster_upgrade_unreachable_count > 0 {
                    Style::default().fg(palette::WARNING_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::ERROR_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD)
                };
                frame
                    .render_widget(Paragraph::new(Line::from(Span::styled(text, badge_style))).style(Style::default().bg(palette::BG_1)), badge_area);
            }

            let offline_paragraph = Paragraph::new(Line::from(vec![Span::styled(
                "  Offline \u{2716} ",
                Style::default().fg(palette::ERROR_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(palette::BG_1));
            frame.render_widget(offline_paragraph, offline_area);
        } else {
            if let Some(text) = badge_text {
                let badge_w = UnicodeWidthStr::width(text) as u16;
                let [main_status, badge_area]: [Rect; 2] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(0), Constraint::Length(badge_w)].as_ref())
                    .split(status_area)
                    .as_ref()
                    .try_into()
                    .unwrap();
                let status_paragraph =
                    Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
                frame.render_widget(status_paragraph, main_status);
                let badge_style = if self.cluster_upgrade_unreachable_count > 0 {
                    Style::default().fg(palette::WARNING_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::ERROR_PEAK).bg(palette::BG_1).add_modifier(Modifier::BOLD)
                };
                frame
                    .render_widget(Paragraph::new(Line::from(Span::styled(text, badge_style))).style(Style::default().bg(palette::BG_1)), badge_area);
            } else {
                let status_paragraph =
                    Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
                frame.render_widget(status_paragraph, status_area);
            }
        }
    }

    fn on_events(&mut self) -> io::Result<()> {
        self.sync_main_focus_for_overlays();
        let poll_dur = if self.repo_manager.progress.lock().unwrap().is_some()
            || self.registration_progress.lock().unwrap().visible
            || self.delete_progress.visible
            || self.cluster_upgrade_progress.visible
            || self.cluster_start_progress.visible
        {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(1)
        };
        if event::poll(poll_dur)? {
            match event::read()? {
                Event::Key(e) if e.kind == KeyEventKind::Press => {
                    self.on_key(e);
                }
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::Down(MouseButton::Left) => self.on_mouse_click(me),
                    MouseEventKind::Moved => self.on_mouse_move(me),
                    _ => {}
                },
                _ => {}
            }
        } else {
            if !self.offline {
                let selected_sid = if self.cycles_buf.is_empty() { None } else { Some(self.get_selected_cycle().event().sid().to_string()) };
                match self.get_cycles() {
                    Ok(cycles) => {
                        self.cycles_buf = cycles;
                        if let Some(selected_sid) = selected_sid
                            && let Some(idx) = self.cycles_buf.iter().position(|cycle| cycle.event().sid() == selected_sid)
                        {
                            self.selected_cycle = idx;
                        } else if self.selected_cycle >= self.cycles_buf.len() {
                            self.selected_cycle = self.cycles_buf.len().saturating_sub(1);
                        }
                    }
                    Err(_) => {
                        self.offline = true;
                        self.evtipc = None;
                    }
                }
                if !self.offline {
                    self.refresh_cluster_upgrade_status();
                    if self.minions_visible {
                        self.refresh_minions();
                    }
                    self.refresh_selected_cycle_contents_preserve_selection();
                    if self.minion_logs_visible && self.minion_logs_polling && self.minion_logs_last_fetch.elapsed() >= Duration::from_secs(3) {
                        match self.load_selected_minion_logs() {
                            Ok(()) => self.minion_logs_online = true,
                            Err(_) => self.minion_logs_online = false,
                        }
                    }
                    if self.systop.visible
                        && self.systop.last_fetch.is_none_or(|last| last.elapsed() >= Duration::from_secs(1))
                        && let Err(err) = self.load_selected_minion_top()
                    {
                        self.log_systop_refresh_error(&err);
                    }
                    if self.master_logs_visible && self.master_logs_polling && self.master_logs_last_fetch.elapsed() >= Duration::from_secs(3) {
                        let _ = self.load_master_logs();
                    }
                }
            }
        }
        if self.delete_progress.visible && self.delete_progress.last_tick.elapsed() >= self.delete_progress.spinner.spinner.fps {
            let tick = self.delete_progress.spinner.tick();
            self.delete_progress.spinner.update(tick);
            self.delete_progress.last_tick = Instant::now();
        }
        if self.cluster_upgrade_progress.visible
            && self.cluster_upgrade_progress.last_tick.elapsed() >= self.cluster_upgrade_progress.spinner.spinner.fps
        {
            let tick = self.cluster_upgrade_progress.spinner.tick();
            self.cluster_upgrade_progress.spinner.update(tick);
            self.cluster_upgrade_progress.last_tick = Instant::now();
        }
        if self.cluster_start_progress.visible && self.cluster_start_progress.last_tick.elapsed() >= self.cluster_start_progress.spinner.spinner.fps {
            let tick = self.cluster_start_progress.spinner.tick();
            self.cluster_start_progress.spinner.update(tick);
            self.cluster_start_progress.last_tick = Instant::now();
        }
        if self.delete_progress.visible
            && self.delete_task.as_ref().is_some_and(|task| task.is_finished())
            && let Some(task) = self.delete_task.take()
        {
            let result = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task));
            self.delete_progress.visible = false;
            self.delete_progress.message.clear();
            self.restore_status();
            match result {
                Ok(Ok(mid)) => {
                    self.info_alert_visible = true;
                    self.info_alert_title = "Minion Removal".to_string();
                    self.info_alert_styled = self.delete_success_styled.take();
                    self.info_alert_message = if self.delete_success_message.is_empty() {
                        Self::format_machine_message("Minion deleted", None, None, Some(&mid))
                    } else {
                        std::mem::take(&mut self.delete_success_message)
                    };
                    self.refresh_minions();
                }
                Ok(Err(err)) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = err;
                }
                Err(err) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Failed to join delete task: {err}");
                }
            }
        }
        if self.cluster_upgrade_progress.visible
            && self.cluster_upgrade_task.as_ref().is_some_and(|task| task.is_finished())
            && let Some(task) = self.cluster_upgrade_task.take()
        {
            let result = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task));
            self.cluster_upgrade_progress.visible = false;
            self.cluster_upgrade_progress.message.clear();
            self.restore_status();
            self.refresh_cluster_upgrade_status();
            self.refresh_minions();
            match result {
                Ok(Ok((updated, dispatched, skipped, failed, offline, items))) => {
                    self.info_alert_visible = true;
                    self.info_alert_title = "Cluster Upgrade".to_string();
                    self.info_alert_styled = None;
                    self.info_alert_message = format!(
                        "SSH upgraded:         {updated}\nDispatched to online: {dispatched}\nSkipped:              {skipped}\nFailed:               {failed}\nOffline:              {offline}{}",
                        if items.is_empty() { String::new() } else { format!("\n\n{}", items.join("\n")) }
                    );
                }
                Ok(Err(err)) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = err;
                }
                Err(err) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Failed to join cluster upgrade task: {err}");
                }
            }
        }
        if self.cluster_start_progress.visible
            && self.cluster_start_task.as_ref().is_some_and(|task| task.is_finished())
            && let Some(task) = self.cluster_start_task.take()
        {
            let result = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task));
            self.cluster_start_progress.visible = false;
            self.cluster_start_progress.message.clear();
            self.restore_status();
            self.status_at_minions_browser();
            match result {
                Ok(Ok((count, items))) => {
                    let failed = items.len();
                    let ok = count.saturating_sub(failed);
                    if failed == 0 {
                        self.info_alert_message = format!("All {ok} minion(s) started successfully");
                    } else {
                        self.info_alert_message = format!("{ok}/{count} minions started");
                    }
                    self.info_alert_title = "Cluster Start".to_string();
                    self.info_alert_visible = true;
                }
                Ok(Err(err)) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = err;
                }
                Err(err) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Cluster start task failed: {err}");
                }
            }
        }
        // Process file picker result for repo manager
        if self.repo_manager.visible
            && let Some(path) = self.file_picker.selected.take()
        {
            match self.repo_manager.active_tab {
                0 => self.process_module_add(&path),
                1 => self.process_library_add(&path),
                2 => self.process_model_add(&path),
                4 => self.process_platform_add(&path),
                _ => {}
            }
        }
        // Detect progress bar completion for bulk add
        if self.repo_manager.visible {
            let p = self.repo_manager.progress.lock().unwrap();
            if p.is_none() {
                drop(p);
                if self.repo_manager.needs_reload {
                    self.repo_manager.needs_reload = false;
                    let _ = self.load_module_index();
                    let _ = self.load_model_list();
                    let _ = self.load_library_index();
                    if self.repo_manager.active_tab == 4 {
                        let _ = self.load_platforms();
                    }
                    self.repo_manager.profiles.has_global_modules.set(self.repo_manager.module_groups.values().any(|v| !v.is_empty()));
                    self.repo_manager.profiles.has_global_models.set(!self.repo_manager.model_rows.is_empty());
                    self.mark_repo_sync_pending();
                }
            } else {
                // Track that a reload is needed when progress finishes
                self.repo_manager.needs_reload = true;
            }
        }
        // Registration completion check (only auto-dismiss on success)
        if self.registration_progress.lock().unwrap().visible && self.registration_progress.lock().unwrap().done {
            let p = self.registration_progress.lock().unwrap();
            let has_error = p.error.is_some();
            if !has_error {
                let host = p.host_label.trim();
                let host_value = if !host.is_empty() {
                    Some(host)
                } else if !p.host.trim().is_empty() {
                    Some(p.host.trim())
                } else {
                    None
                };
                let platform = (!p.platform.trim().is_empty()).then_some(p.platform.trim());
                let msg = Self::format_machine_message("Minion registered", host_value, platform, p.minion_id.as_deref());
                let styled = Self::format_machine_message_styled("Minion registered", host_value, platform, p.minion_id.as_deref());
                drop(p);
                *self.registration_progress.lock().unwrap() = minreg::RegistrationProgress::placeholder();
                self.restore_status();
                self.info_alert_visible = true;
                self.info_alert_title = "Minion Registration".to_string();
                self.info_alert_styled = Some(styled);
                self.info_alert_message = msg;
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
                    self.help_popup_scroll.set(0);
                }
                KeyCode::Up => {
                    self.help_popup_scroll.set(self.help_popup_scroll.get().saturating_sub(1));
                }
                KeyCode::Down => {
                    self.help_popup_scroll.set(self.help_popup_scroll.get().saturating_add(1));
                }
                KeyCode::PageUp => {
                    self.help_popup_scroll.set(self.help_popup_scroll.get().saturating_sub(10));
                }
                KeyCode::PageDown => {
                    self.help_popup_scroll.set(self.help_popup_scroll.get().saturating_add(10));
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

        if self.systop.visible {
            return self.on_systop_popup(e);
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
                let len = Self::minions_menu_len();
                if len > 0 {
                    self.minions_menu_sel = if self.minions_menu_sel == 0 { len - 1 } else { self.minions_menu_sel - 1 };
                }
            }
            KeyCode::Down => {
                let len = Self::minions_menu_len();
                if len > 0 {
                    self.minions_menu_sel = if self.minions_menu_sel >= len - 1 { 0 } else { self.minions_menu_sel + 1 };
                }
            }
            KeyCode::PageUp => {
                self.minions_menu_sel = self.minions_menu_sel.saturating_sub(3);
            }
            KeyCode::PageDown => {
                let len = Self::minions_menu_len();
                if len > 0 {
                    self.minions_menu_sel = (self.minions_menu_sel + 3).min(len - 1);
                }
            }
            KeyCode::Enter => {
                self.minions_menu_visible = false;
                if self.minions_menu_sel == 0 {
                    self.open_logs_popup();
                } else if self.minions_menu_sel == 1 {
                    self.open_traits_popup();
                } else if self.minions_menu_sel == 2 {
                    self.open_systop_popup();
                } else if self.minions_menu_sel == 3 {
                    self.do_minion_start();
                } else if self.minions_menu_sel == 4 {
                    self.do_minion_shutdown();
                } else if self.minions_menu_sel == 5 {
                    self.do_minion_reconnect();
                } else if self.minions_menu_sel == 6 {
                    self.open_cluster_confirm(3);
                } else if self.minions_menu_sel == 7 {
                    self.open_cluster_confirm(4);
                } else if self.minions_menu_sel == 8 {
                    self.open_cluster_confirm(1);
                } else if self.minions_menu_sel == 9 {
                    self.open_cluster_confirm(2);
                } else if self.minions_menu_sel == 10 {
                    self.registration_form.visible = true;
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
            || self.info_alert_visible
            || self.help_popup_visible
            || self.minions_visible
            || self.minion_traits_visible
            || self.minion_logs_visible
            || self.systop.visible
            || self.master_logs_visible
            || self.minions_menu_visible
            || self.master_menu_visible
            || self.master_confirm_visible
            || self.tag_visible
            || self.dsl_browser.visible
            || self.setup_wizard.visible
            || self.file_picker.visible
            || self.repo_manager.visible
            || self.repo_manager.profiles.detail_visible
            || self.repo_manager.profiles.create_visible
            || self.repo_manager.profiles.delete_visible
            || self.repo_manager.platforms.delete_visible
            || self.registration_form.visible
            || self.registration_progress.lock().unwrap().visible
            || self.delete_progress.visible
            || self.cluster_upgrade_progress.visible
            || self.cluster_start_progress.visible
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

    fn refresh_cluster_upgrade_status(&mut self) {
        match self.fetch_cluster_upgrade_status() {
            Ok((required, unreachable, pending)) => {
                self.cluster_upgrade_required_count = required;
                self.cluster_upgrade_unreachable_count = unreachable;
                self.cluster_upgrade_pending_count = pending;
            }
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot fetch upgrade status: {e}");
            }
        }
    }

    fn mark_cluster_upgrade_required(&mut self) {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MARK_UPGRADE_REQUIRED}"), "*", None, None, None).await
            })
        }) {
            Ok(resp) => {
                self.refresh_cluster_upgrade_status();
                if self.minions_visible {
                    self.refresh_minions();
                }
                if let ConsolePayload::Ack { count, items, .. } = &resp.payload {
                    self.cluster_upgrade_check_message =
                        Some(format!("{} marked, {}", count, items.first().map(|s| s.as_str()).unwrap_or("no response")));
                }
            }
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot mark cluster upgrade: {e}");
            }
        }
    }

    fn mark_repo_sync_pending(&mut self) {
        self.repo_manager.pending_cluster_upgrade = true;
    }

    fn start_cluster_upgrade(&mut self) {
        self.cluster_upgrade_progress.visible = true;
        self.cluster_upgrade_progress.message = "Applying repository updates across the cluster...".to_string();
        self.cluster_upgrade_progress.last_tick = Instant::now();
        self.status_text = Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(palette::FG)),
            Span::styled("wait for completion", Style::default().fg(palette::FAINT)),
        ]);
        let cfg = self.cfg.clone();
        self.cluster_upgrade_task = Some(tokio::spawn(async move {
            call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_UPGRADE_MINIONS}"), "*", None, None, None)
                .await
                .map_err(|err| err.to_string())
                .and_then(|resp| match resp.payload {
                    ConsolePayload::UpgradeSummary { updated, dispatched, skipped, failed, offline, items } => {
                        Ok((updated, dispatched, skipped, failed, offline, items))
                    }
                    _ => Err("Unexpected console payload for cluster upgrade".to_string()),
                })
        }));
    }

    fn start_cluster_sync(&mut self) {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_SYNC}"), "*", None, None, None).await })
        }) {
            Ok(resp) => {
                self.refresh_cluster_upgrade_status();
                self.refresh_minions();
                match resp.payload {
                    ConsolePayload::Ack { items, .. } => {
                        self.info_alert_visible = true;
                        self.info_alert_title = "Cluster Sync".to_string();
                        self.info_alert_styled = None;
                        self.info_alert_message = if items.is_empty() {
                            "Cluster sync dispatched".to_string()
                        } else {
                            format!("Cluster sync dispatched\n\n{}", items.join("\n"))
                        };
                    }
                    other => {
                        self.error_alert_visible = true;
                        self.error_alert_message = format!("Unexpected console payload for cluster sync: {other:?}");
                    }
                }
            }
            Err(err) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Failed to run cluster sync: {err}");
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

    fn log_systop_refresh_error(&self, err: &SysinspectError) {
        let base = if err.to_string().contains("exceeds") { "System Top exceeded frame size" } else { "System Top refresh failed" };
        log::warn!("{base} for {} ({}): {err}; skipping this frame", self.systop.minion_id, self.systop.host);
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
            KeyCode::Char('p') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_systop_popup();
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
                self.open_cluster_confirm(1);
                true
            }
            KeyCode::Char('h') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_cluster_confirm(4);
                true
            }
            KeyCode::Char('a') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_cluster_confirm(2);
                true
            }
            KeyCode::Insert => {
                self.registration_form.visible = true;
                true
            }
            KeyCode::Delete => {
                self.open_cluster_confirm(3);
                true
            }
            _ => false,
        }
    }

    fn on_cluster_confirm(&mut self, e: event::KeyEvent) -> bool {
        if !self.cluster_confirm_visible {
            return false;
        }
        if self.pending_cluster_action == 3 {
            match e.code {
                KeyCode::Tab => {
                    self.cluster_confirm_form_focus = self.cluster_confirm_form_focus.next(1, true);
                }
                KeyCode::BackTab => {
                    self.cluster_confirm_form_focus = self.cluster_confirm_form_focus.prev(1, true);
                }
                KeyCode::Char(' ') => {
                    if matches!(self.cluster_confirm_form_focus, DialogFormFocus::Widget(0)) {
                        self.delete_force_remove = !self.delete_force_remove;
                    }
                }
                KeyCode::Enter => match self.cluster_confirm_form_focus {
                    DialogFormFocus::Widget(0) => {
                        self.delete_force_remove = !self.delete_force_remove;
                    }
                    DialogFormFocus::LeftButton => {
                        let force = self.delete_force_remove;
                        self.close_cluster_confirm();
                        self.do_minion_delete(force);
                    }
                    DialogFormFocus::RightButton => {
                        self.close_cluster_confirm();
                    }
                    DialogFormFocus::Widget(_) => {}
                },
                KeyCode::Esc => {
                    self.close_cluster_confirm();
                }
                _ => {}
            }
            return true;
        }
        match e.code {
            KeyCode::Tab => {
                if self.cluster_confirm_choice == AlertResult::Default {
                    self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                } else {
                    self.cluster_confirm_choice = AlertResult::Default;
                }
            }
            KeyCode::Char(' ') => {
                if self.pending_cluster_action == 3 {
                    self.delete_force_remove = !self.delete_force_remove;
                }
            }
            KeyCode::Enter => {
                self.cluster_confirm_visible = false;
                if self.cluster_confirm_choice == AlertResult::ClusterConfirm {
                    match self.pending_cluster_action {
                        1 => self.do_cluster_shutdown(),
                        2 => self.do_cluster_reconnect(),
                        3 => self.do_minion_delete(self.delete_force_remove),
                        4 => self.do_cluster_hopstart(),
                        _ => {}
                    }
                }
                self.pending_cluster_action = 0;
                self.delete_force_remove = false;
                self.cluster_confirm_form_focus = DialogFormFocus::LeftButton;
                self.status_at_minions_browser();
            }
            KeyCode::Esc => {
                self.close_cluster_confirm();
            }
            _ => {}
        }
        true
    }

    fn open_cluster_confirm(&mut self, action: u8) {
        self.cluster_confirm_visible = true;
        self.cluster_confirm_choice = AlertResult::ClusterConfirm;
        self.pending_cluster_action = action;
        self.cluster_confirm_form_focus = if action == 3 { DialogFormFocus::RightButton } else { DialogFormFocus::LeftButton };
    }

    fn close_cluster_confirm(&mut self) {
        self.cluster_confirm_visible = false;
        self.pending_cluster_action = 0;
        self.delete_force_remove = false;
        self.cluster_confirm_form_focus = DialogFormFocus::LeftButton;
        self.status_at_minions_browser();
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

    fn do_cluster_hopstart(&mut self) {
        self.cluster_start_progress.visible = true;
        self.cluster_start_progress.message = "Booting cluster, please wait...".to_string();
        self.cluster_start_progress.last_tick = Instant::now();
        self.status_text = Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(palette::FG)),
            Span::styled("wait for completion", Style::default().fg(palette::FAINT)),
        ]);
        let cfg = self.cfg.clone();
        self.cluster_start_task = Some(tokio::spawn(async move {
            call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_HOPSTART}"), "*", None, None, None)
                .await
                .map_err(|err| err.to_string())
                .and_then(|resp| match resp.payload {
                    ConsolePayload::Ack { count, items, .. } => Ok((count, items)),
                    _ => Err("Unexpected console payload for cluster start".to_string()),
                })
        }));
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

    fn do_minion_delete(&mut self, force: bool) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion selected".to_string();
                self.status_at_minions_browser();
                return;
            }
        };
        let mid = row.minion_id.clone();
        let host_label = Self::online_host(&row);
        let host = if !host_label.trim().is_empty() && host_label != "unknown" {
            Some(host_label)
        } else if !row.ip.trim().is_empty() {
            Some(row.ip.clone())
        } else {
            None
        };
        let platform: Option<String> = if !row.os_name.trim().is_empty() {
            if row.os_version.trim().is_empty() {
                Some(os_display_name(&row.os_name).to_string())
            } else {
                Some(format!("{} {}", os_display_name(&row.os_name), row.os_version.trim()))
            }
        } else if !row.os_distribution.trim().is_empty() {
            Some(row.os_distribution.clone())
        } else {
            None
        };
        self.delete_success_message = Self::format_machine_message("Minion deleted", host.as_deref(), platform.as_deref(), Some(&mid));
        self.delete_success_styled = Some(Self::format_machine_message_styled("Minion deleted", host.as_deref(), platform.as_deref(), Some(&mid)));
        let force_ctx = if force { Some(serde_json::json!({"force": true}).to_string()) } else { None };
        self.delete_progress.visible = true;
        self.delete_progress.message =
            if force { "Removing client from host, please wait...".to_string() } else { "Removing client, please wait...".to_string() };
        self.delete_progress.last_tick = Instant::now();
        self.delete_task = Some(tokio::spawn({
            let cfg = self.cfg.clone();
            async move {
                match call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "*", None, Some(&mid), force_ctx.as_ref()).await
                {
                    Ok(rsp) if matches!(rsp.payload, ConsolePayload::Ack { .. }) => Ok(mid),
                    Ok(_) => Err("Master did not acknowledge minion removal.".to_string()),
                    Err(err) => Err(format!("Failed to delete minion: {err}")),
                }
            }
        }));
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

    fn open_systop_popup(&mut self) {
        let row = match self.selected_popup_minion() {
            Some(row) => row,
            None => {
                self.error_alert_visible = true;
                self.error_alert_message = "No minion is currently selected".to_string();
                return;
            }
        };
        self.systop.open(row.minion_id.clone(), Self::online_host(&row));
        self.status_at_systop();
        if let Err(err) = self.load_selected_minion_top() {
            self.systop.close();
            self.error_alert_visible = true;
            self.error_alert_message = err.to_string();
            self.status_at_minions_browser();
        }
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

    fn load_selected_minion_top(&mut self) -> Result<(), SysinspectError> {
        let row = self.selected_popup_minion().ok_or_else(|| SysinspectError::InvalidQuery("No minion is currently selected".to_string()))?;
        let context = serde_json::json!({"process_limit": 64usize}).to_string();
        let mid = row.minion_id.clone();
        let snapshot = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_TOP}"), "*", None, Some(&mid), Some(&context)).await
            })
        })
        .and_then(|resp| match resp.payload {
            ConsolePayload::MinionTop { snapshot } => Ok(snapshot),
            _ => Err(SysinspectError::ProtoError("Unexpected console payload for minion system top".to_string())),
        })?;
        self.systop.apply_snapshot(snapshot);
        Ok(())
    }

    fn signal_selected_systop_process(&mut self, pid: u32, signal: i32) -> Result<(), SysinspectError> {
        let context = serde_json::json!({"pid": pid, "signal": signal}).to_string();
        let mid = self.systop.minion_id.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_PROCESS_SIGNAL}"), "*", None, Some(&mid), Some(&context))
                    .await
            })
        })?;
        self.load_selected_minion_top()?;
        Ok(())
    }

    fn on_systop_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.systop.visible {
            return false;
        }
        if self.systop.process_shootout_visible {
            match e.code {
                KeyCode::Esc => {
                    self.systop.close_process_shootout();
                }
                KeyCode::Up => {
                    self.systop.process_shootout_sel = self.systop.process_shootout_sel.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.systop.process_shootout_sel = (self.systop.process_shootout_sel + 1).min(2);
                }
                KeyCode::Enter => {
                    if let Some((pid, signal, _)) = self.systop.selected_process_action() {
                        match self.signal_selected_systop_process(pid, signal) {
                            Ok(()) => self.systop.close_process_shootout(),
                            Err(err) => {
                                self.systop.close_process_shootout();
                                let _ = self.load_selected_minion_top();
                                self.error_alert_visible = true;
                                self.error_alert_message = err.to_string();
                            }
                        }
                    }
                }
                _ => {}
            }
            return true;
        }

        match e.code {
            KeyCode::Esc => {
                if self.systop.process_filter_active {
                    self.systop.clear_process_filter();
                    return true;
                }
                self.persist_system_top_preferences();
                self.systop.close();
                self.status_at_minions_browser();
            }
            KeyCode::Up => {
                self.systop.move_process_selection(-1);
            }
            KeyCode::Down => {
                self.systop.move_process_selection(1);
            }
            KeyCode::PageUp => {
                self.systop.page_process_selection(-1);
            }
            KeyCode::PageDown => {
                self.systop.page_process_selection(1);
            }
            KeyCode::Left => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Left);
                } else {
                    self.systop.set_chart_mode(crate::ui::systop::ChartMode::Blocks);
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Right => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Right);
                } else {
                    self.systop.set_chart_mode(crate::ui::systop::ChartMode::Line);
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Tab => {
                if self.systop.process_filter_active {
                    self.systop.cycle_process_filter_focus(true);
                } else {
                    self.systop.cycle_network_interface(true);
                }
            }
            KeyCode::BackTab => {
                if self.systop.process_filter_active {
                    self.systop.cycle_process_filter_focus(false);
                } else {
                    self.systop.cycle_network_interface(false);
                }
            }
            KeyCode::Enter => {
                if self.systop.snapshot.is_some() {
                    self.systop.open_process_shootout();
                }
            }
            KeyCode::Backspace => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Backspace);
                    self.systop.move_process_selection(0);
                }
            }
            KeyCode::Delete => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Delete);
                    self.systop.move_process_selection(0);
                }
            }
            KeyCode::Home => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Home);
                } else {
                    self.systop.process_selected = 0;
                    self.systop.move_process_selection(0);
                }
            }
            KeyCode::End => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::End);
                } else {
                    if let Some(snapshot) = &self.systop.snapshot {
                        self.systop.process_selected = self.systop.filtered_processes(snapshot).len().saturating_sub(1);
                        self.systop.move_process_selection(0);
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Char('c'));
                    self.systop.move_process_selection(0);
                } else {
                    self.systop.apply_sort_key('c');
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Char('m'));
                    self.systop.move_process_selection(0);
                } else {
                    self.systop.apply_sort_key('m');
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Char('p'));
                    self.systop.move_process_selection(0);
                } else {
                    self.systop.apply_sort_key('p');
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if self.systop.process_filter_active {
                    self.systop.edit_process_filter(KeyCode::Char('n'));
                    self.systop.move_process_selection(0);
                } else {
                    self.systop.apply_sort_key('n');
                    self.persist_system_top_preferences();
                }
            }
            KeyCode::Char('/') if !e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.systop.activate_process_filter();
            }
            KeyCode::Char(c) if self.systop.process_filter_active => {
                self.systop.edit_process_filter(KeyCode::Char(c));
                self.systop.move_process_selection(0);
            }
            _ => {}
        }
        true
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

    fn open_master_logs(&mut self) {
        self.master_logs_visible = true;
        self.master_logs_tab = 0;
        self.master_logs_filter = ratatui_cheese::input::InputState::new();
        self.master_logs_filter_focus = false;
        self.master_logs_polling = true;
        self.master_logs_last_fetch = Instant::now();
        self.status_at_master_logs();
        if let Err(err) = self.load_master_logs() {
            self.master_logs_visible = false;
            self.error_alert_visible = true;
            self.error_alert_message = err.to_string();
        }
    }

    fn load_master_logs(&mut self) -> Result<(), SysinspectError> {
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MASTER_LOGS}"), "*", None, None, None).await })
        })?;
        match resp.payload {
            ConsolePayload::MasterLogs { snapshot } => {
                self.master_logs_sections = vec![
                    rawlogs::LogSection {
                        title: "Standard".into(),
                        path: snapshot.standard_path,
                        lines: snapshot.standard_log,
                        scroll: Cell::new(usize::MAX),
                    },
                    rawlogs::LogSection {
                        title: "Errors".into(),
                        path: snapshot.errors_path,
                        lines: snapshot.errors_log,
                        scroll: Cell::new(usize::MAX),
                    },
                ];
                self.master_logs_last_fetch = Instant::now();
                Ok(())
            }
            _ => Err(SysinspectError::ProtoError("Unexpected console payload for master logs".to_string())),
        }
    }

    fn load_master_logs_local(&mut self) -> Result<(), String> {
        let std = std::fs::read_to_string(self.cfg.logfile_std()).map_err(|e| format!("Cannot read standard log: {e}"))?;
        let err = std::fs::read_to_string(self.cfg.logfile_err()).map_err(|e| format!("Cannot read error log: {e}"))?;
        self.master_logs_sections = vec![
            rawlogs::LogSection {
                title: "Standard".into(),
                path: self.cfg.logfile_std().display().to_string(),
                lines: if std.is_empty() { vec!["(empty)".into()] } else { std.lines().map(|s| s.to_string()).collect() },
                scroll: Cell::new(usize::MAX),
            },
            rawlogs::LogSection {
                title: "Errors".into(),
                path: self.cfg.logfile_err().display().to_string(),
                lines: if err.is_empty() { vec!["(empty)".into()] } else { err.lines().map(|s| s.to_string()).collect() },
                scroll: Cell::new(usize::MAX),
            },
        ];
        self.master_logs_last_fetch = Instant::now();
        Ok(())
    }

    fn load_module_index(&mut self) -> Result<(), String> {
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MODULE_INDEX}"), "*", None, None, None).await })
        })
        .map_err(|e| format!("Failed to get module index: {e}"))?;
        match resp.payload {
            ConsolePayload::MasterModuleIndex { rows } => {
                let mut groups: IndexMap<String, Vec<ConsoleModuleRow>> = IndexMap::new();
                for row in rows {
                    let key = format!("{} {}", os_display_name(&row.platform), row.arch);
                    groups.entry(key).or_default().push(row);
                }
                // Merge platforms with no modules from the platform build list
                if let Ok(repo) = SysInspectModPak::new(self.cfg.fileserver_root().join("repo")) {
                    for build in repo.minion_builds() {
                        let key = format!("{} {}", os_display_name(build.platform()), build.arch());
                        groups.entry(key).or_default();
                    }
                }
                let mut keys: Vec<String> = groups.keys().cloned().collect();
                keys.sort();
                for rows in groups.values_mut() {
                    rows.sort_by(|a, b| a.name.cmp(&b.name));
                }
                let n = groups.len();
                self.repo_manager.module_groups = groups;
                self.repo_manager.group_order = keys;
                self.repo_manager.group_cursor = 0;
                self.repo_manager.group_cursor_row = 0;
                self.repo_manager.group_expanded = vec![false; n];
                self.repo_manager.group_scrolls.clear();
                Ok(())
            }
            _ => Err("Unexpected console payload for module index".to_string()),
        }
    }

    fn on_repo_manager(&mut self, e: event::KeyEvent) -> bool {
        if !self.repo_manager.visible {
            return false;
        }
        if self.repo_manager.staging {
            let handled = self.repo_manager.handle_staging_key(e);
            if self.repo_manager.bulk_add_triggered {
                self.repo_manager.bulk_add_triggered = false;
                let checked: Vec<_> = self.repo_manager.staged.iter().filter(|m| m.checked).cloned().collect();
                if checked.is_empty() {
                    self.error_alert_visible = true;
                    self.error_alert_message = "No items selected".to_string();
                } else {
                    self.repo_manager.exit_staging();
                    match self.repo_manager.staging_mode {
                        repomanager::StagingMode::ProfileModuleAdd => {
                            self.bulk_add_profile_matches(checked, false, false);
                        }
                        repomanager::StagingMode::ProfileModelAdd => {
                            self.bulk_add_profile_matches(checked.clone(), false, false);
                            let model_ids: Vec<String> = checked.iter().map(|m| m.name.clone()).collect();
                            let profile_name = self.repo_manager.profiles.detail_name.clone();
                            let _ = self.do_profile_add_matches(&profile_name, model_ids, false, true);
                            match self.load_profile_detail(&profile_name) {
                                Ok((models, modules, model_groups, ungrouped_modules, libraries)) => {
                                    self.repo_manager.profiles.enter_detail(
                                        profile_name,
                                        models,
                                        modules,
                                        model_groups,
                                        ungrouped_modules,
                                        libraries,
                                    );
                                }
                                Err(e) => {
                                    self.error_alert_visible = true;
                                    self.error_alert_message = e;
                                }
                            }
                        }
                        repomanager::StagingMode::ProfileLibraryAdd => {
                            self.bulk_add_profile_matches(checked, true, false);
                        }
                        _ => {
                            self.bulk_add_modules(checked);
                        }
                    }
                }
            }
            if self.repo_manager.bulk_delete_triggered {
                self.repo_manager.bulk_delete_triggered = false;
                let checked: Vec<_> = self.repo_manager.staged.iter().filter(|m| m.checked).cloned().collect();
                if checked.is_empty() {
                    self.error_alert_visible = true;
                    self.error_alert_message = "No items selected".to_string();
                } else {
                    let staging_mode = self.repo_manager.staging_mode;
                    let cross_platform_delete = self.repo_manager.cross_platform_delete;
                    self.repo_manager.exit_staging();
                    match staging_mode {
                        repomanager::StagingMode::LibraryDelete => {
                            let names: Vec<String> = checked.iter().map(|m| m.name.clone()).collect();
                            self.bulk_delete_libraries(&names);
                        }
                        repomanager::StagingMode::ModuleDelete => {
                            if cross_platform_delete {
                                let names: Vec<String> = checked.iter().map(|m| m.name.clone()).collect();
                                self.bulk_delete_modules(&names);
                            } else {
                                self.bulk_delete_single_platform(&checked);
                            }
                        }
                        _ => {
                            let names: Vec<String> = checked.iter().map(|m| m.name.clone()).collect();
                            self.bulk_delete_modules(&names);
                        }
                    }
                }
            }
            if !self.repo_manager.staging
                && self.repo_manager.active_tab == 3
                && matches!(
                    self.repo_manager.staging_mode,
                    repomanager::StagingMode::ProfileModuleAdd
                        | repomanager::StagingMode::ProfileModelAdd
                        | repomanager::StagingMode::ProfileLibraryAdd
                )
            {
                self.repo_manager.profiles.detail_visible = true;
                self.status_at_profiles();
            }
            return handled;
        }
        if self.repo_manager.info_visible {
            return self.repo_manager.handle_info_key(e);
        }
        if self.repo_manager.model_delete_visible {
            let handled = self.repo_manager.handle_model_delete_key(e);
            if !handled && e.code == KeyCode::Enter {
                if self.repo_manager.model_delete_focus == repomanager::ModelDeleteFocus::YesBtn {
                    let model_id = self.repo_manager.model_delete_id.clone();
                    match self.delete_model(&model_id) {
                        Ok(()) => {
                            let _ = self.load_model_list();
                            self.info_alert_visible = true;
                            self.info_alert_title = "Model Removal".to_string();
                            self.info_alert_styled = None;
                            self.info_alert_message = format!("Model removed: {model_id}");
                        }
                        Err(err) => {
                            self.error_alert_visible = true;
                            self.error_alert_message = err;
                        }
                    }
                }
                self.repo_manager.model_delete_visible = false;
            }
            return true;
        }
        if self.repo_manager.filter_focus {
            match e.code {
                KeyCode::Esc => {
                    self.repo_manager.filter_focus = false;
                    self.repo_manager.group_cursor_row = 0;
                }
                KeyCode::Down | KeyCode::Tab | KeyCode::BackTab => {
                    self.repo_manager.filter_focus = false;
                    self.repo_manager.group_cursor_row = 0;
                }
                KeyCode::Backspace => {
                    self.repo_manager.filter.delete_before();
                }
                KeyCode::Delete => {
                    self.repo_manager.filter.delete_at();
                }
                KeyCode::Left => {
                    self.repo_manager.filter.move_left();
                }
                KeyCode::Right => {
                    self.repo_manager.filter.move_right();
                }
                KeyCode::Home => {
                    self.repo_manager.filter.home();
                }
                KeyCode::End => {
                    self.repo_manager.filter.end();
                }
                KeyCode::Char(c) => {
                    self.repo_manager.filter.insert_char(c);
                }
                _ => {}
            }
            return true;
        }
        // Profile-specific overlays (tab 3)
        if self.repo_manager.active_tab == 3 {
            if self.repo_manager.profiles.delete_visible {
                let handled = self.repo_manager.profiles.handle_delete_key(e.code);
                if !handled && e.code == KeyCode::Enter {
                    if self.repo_manager.profiles.delete_focus == profiles::ProfDeleteFocus::YesBtn {
                        let name = self.repo_manager.profiles.delete_name.clone();
                        let _ = self.do_profile_delete(&name);
                        self.repo_manager.profiles.delete_visible = false;
                        self.status_at_profiles();
                    } else {
                        self.repo_manager.profiles.delete_visible = false;
                        self.status_at_profiles();
                    }
                }
                return true;
            }
            if self.repo_manager.profiles.assign.visible {
                let handled = self.repo_manager.profiles.handle_assign_key(e.code);
                if !handled && e.code == KeyCode::Enter {
                    let profile_name = self.repo_manager.profiles.assign.profile_name.clone();
                    let selected: Vec<String> =
                        self.repo_manager.profiles.assign.minions.iter().filter(|(_, checked)| *checked).map(|(host, _)| host.clone()).collect();
                    match self.repo_manager.profiles.assign.focus {
                        profiles::ProfAssignFocus::TagBtn if !selected.is_empty() => {
                            let _ = self.do_profile_tag(&profile_name, &selected);
                            self.repo_manager.profiles.assign.visible = false;
                            self.status_at_profiles();
                        }
                        profiles::ProfAssignFocus::UntagBtn if !selected.is_empty() => {
                            let _ = self.do_profile_untag(&profile_name, &selected);
                            self.repo_manager.profiles.assign.visible = false;
                            self.status_at_profiles();
                        }
                        profiles::ProfAssignFocus::CloseBtn => {
                            self.repo_manager.profiles.assign.visible = false;
                            self.status_at_profiles();
                        }
                        _ => {}
                    }
                }
                return true;
            }
            if self.repo_manager.profiles.create_visible {
                let handled = self.repo_manager.profiles.handle_create_key(e.code);
                if !handled && e.code == KeyCode::Enter {
                    match self.repo_manager.profiles.create_focus {
                        profiles::ProfCreateFocus::CreateBtn => {
                            let name = self.repo_manager.profiles.create_input.value().to_string();
                            if !name.is_empty() {
                                let _ = self.do_profile_create(&name);
                            }
                            self.repo_manager.profiles.create_visible = false;
                            self.status_at_profiles();
                        }
                        profiles::ProfCreateFocus::CancelBtn => {
                            self.repo_manager.profiles.create_visible = false;
                            self.status_at_profiles();
                        }
                        _ => {}
                    }
                }
                return true;
            }
            if self.repo_manager.profiles.detail_visible {
                let handled = self.repo_manager.profiles.handle_detail_key(e.code);
                if !handled && e.code == KeyCode::Enter {
                    match self.repo_manager.profiles.detail_focus {
                        profiles::ProfDetailFocus::AddModuleBtn => {
                            self.repo_manager.enter_profile_module_staging();
                        }
                        profiles::ProfDetailFocus::AddFromModelBtn => {
                            if let Err(err) = self.load_model_list() {
                                self.error_alert_visible = true;
                                self.error_alert_message = err;
                                return true;
                            }
                            self.repo_manager.enter_profile_model_staging();
                        }
                        profiles::ProfDetailFocus::AddLibraryBtn => {
                            self.repo_manager.enter_profile_library_staging();
                        }
                        profiles::ProfDetailFocus::CloseBtn => {
                            self.repo_manager.profiles.detail_visible = false;
                            self.repo_manager.staging_mode = repomanager::StagingMode::ModuleAdd;
                            self.status_at_profiles();
                        }
                        profiles::ProfDetailFocus::AssignBtn => {
                            self.repo_manager.profiles.has_connected_minions.set(!self.minions_rows.is_empty());
                            if let Some(name) = self.repo_manager.profiles.selected_profile_name().map(|s| s.to_string()) {
                                self.repo_manager.profiles.assign.minions = self.minions_rows.iter().map(|m| (m.hostname.clone(), false)).collect();
                                self.repo_manager.profiles.assign.profile_name = name;
                                self.repo_manager.profiles.assign.visible = true;
                                self.status_at_profiles();
                            }
                        }
                        _ => {}
                    }
                }
                return true;
            }
        }
        // Platform delete overlay (tab 4)
        if self.repo_manager.active_tab == 4 && self.repo_manager.platforms.delete_visible {
            let handled = self.repo_manager.platforms.handle_delete_key(e.code);
            if !handled && e.code == KeyCode::Enter {
                if self.repo_manager.platforms.delete_focus == platforms::DeleteFocus::YesBtn {
                    let name = self.repo_manager.platforms.delete_name.clone();
                    self.do_platform_remove(&name);
                }
                self.repo_manager.platforms.delete_visible = false;
            }
            return true;
        }
        let total_count = if self.repo_manager.active_tab == 4 {
            self.repo_manager.platforms.filtered_count(self.repo_manager.filter.value())
        } else if self.repo_manager.active_tab == 3 {
            self.repo_manager.profiles.filtered_count(self.repo_manager.filter.value())
        } else if self.repo_manager.active_tab == 2 {
            self.repo_filtered_model_count()
        } else if self.repo_manager.active_tab == 1 {
            self.repo_filtered_lib_count()
        } else if self.repo_manager.active_tab == 0 {
            if let Some(rows) = self.repo_manager.focused_group_modules() {
                let f = self.repo_manager.filter.value().to_lowercase();
                rows.iter().filter(|r| f.is_empty() || r.name.to_lowercase().contains(&f) || r.descr.to_lowercase().contains(&f)).count()
            } else {
                0
            }
        } else {
            self.repo_filtered_count()
        };
        let max_cursor = total_count.saturating_sub(1);
        let cursor_ref: &mut usize = if self.repo_manager.active_tab == 4 {
            &mut self.repo_manager.platforms.cursor
        } else if self.repo_manager.active_tab == 3 {
            &mut self.repo_manager.profiles.cursor
        } else if self.repo_manager.active_tab == 2 {
            &mut self.repo_manager.model_cursor
        } else if self.repo_manager.active_tab == 1 {
            &mut self.repo_manager.lib_cursor
        } else {
            &mut self.repo_manager.group_cursor_row
        };
        let page = 10usize;
        match e.code {
            KeyCode::Esc => {
                if self.repo_manager.models_dirty {
                    if let Err(err) = self.reload_master_config() {
                        self.error_alert_visible = true;
                        self.error_alert_message = err;
                        return true;
                    }
                    self.repo_manager.models_dirty = false;
                }
                let start_repo_sync = self.repo_manager.pending_cluster_upgrade;
                self.repo_manager.pending_cluster_upgrade = false;
                self.repo_manager.exit_staging();
                self.repo_manager.visible = false;
                self.status_at_cycles();
                if start_repo_sync {
                    self.start_cluster_sync();
                }
            }
            KeyCode::Left => {
                self.repo_manager.active_tab = self.repo_manager.active_tab.saturating_sub(1);
                self.repo_manager.group_cursor = 0;
                self.repo_manager.group_cursor_row = 0;
                self.repo_manager.lib_cursor = 0;
                self.repo_manager.model_cursor = 0;
                self.repo_manager.profiles.cursor = 0;
                self.repo_manager.platforms.cursor = 0;
                if self.repo_manager.active_tab == 1 {
                    let _ = self.load_library_index();
                }
                if self.repo_manager.active_tab == 2 && !self.repo_manager.models_dirty {
                    let _ = self.load_model_list();
                }
                if self.repo_manager.active_tab == 3 {
                    let _ = self.load_profile_list();
                }
                if self.repo_manager.active_tab == 4 {
                    let _ = self.load_platforms();
                }
            }
            KeyCode::Right => {
                self.repo_manager.active_tab = (self.repo_manager.active_tab + 1).min(4);
                self.repo_manager.group_cursor = 0;
                self.repo_manager.group_cursor_row = 0;
                self.repo_manager.lib_cursor = 0;
                self.repo_manager.model_cursor = 0;
                self.repo_manager.profiles.cursor = 0;
                self.repo_manager.platforms.cursor = 0;
                if self.repo_manager.active_tab == 1 {
                    let _ = self.load_library_index();
                }
                if self.repo_manager.active_tab == 2 && !self.repo_manager.models_dirty {
                    let _ = self.load_model_list();
                }
                if self.repo_manager.active_tab == 3 {
                    let _ = self.load_profile_list();
                }
                if self.repo_manager.active_tab == 4 {
                    let _ = self.load_platforms();
                }
            }
            KeyCode::Up => {
                if self.repo_manager.active_tab == 0 {
                    self.move_module_up();
                } else if self.repo_manager.active_tab == 3 {
                    let fv = self.repo_manager.filter.value().to_string();
                    self.repo_manager.profiles.handle_list_key(e.code, &mut self.repo_manager.filter_focus, &fv);
                } else if self.repo_manager.active_tab == 4 {
                    self.repo_manager.platforms.handle_list_key(e.code);
                } else {
                    *cursor_ref = cursor_ref.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if self.repo_manager.active_tab == 0 {
                    self.move_module_down();
                } else if self.repo_manager.active_tab == 3 {
                    let fv = self.repo_manager.filter.value().to_string();
                    self.repo_manager.profiles.handle_list_key(e.code, &mut self.repo_manager.filter_focus, &fv);
                } else if self.repo_manager.active_tab == 4 {
                    self.repo_manager.platforms.handle_list_key(e.code);
                } else {
                    *cursor_ref = (*cursor_ref + 1).min(max_cursor);
                }
            }
            KeyCode::Char(' ') if self.repo_manager.active_tab == 2 && !self.repo_manager.model_rows.is_empty() => {
                if let Some(model) = self.repo_manager.model_rows.get(self.repo_manager.model_cursor) {
                    let model_id = model.id.clone();
                    let enabled = !model.enabled;
                    if let Err(err) = self.set_model_enabled(&model_id, enabled) {
                        self.error_alert_visible = true;
                        self.error_alert_message = err;
                    } else {
                        let _ = self.load_model_list();
                    }
                }
            }
            KeyCode::PageUp => {
                if self.repo_manager.active_tab == 0 {
                    let n = self.repo_manager.group_order.len();
                    if n > 0 {
                        self.repo_manager.group_cursor = (self.repo_manager.group_cursor + n - 1) % n;
                        self.repo_manager.group_cursor_row = 0;
                    }
                } else if self.repo_manager.active_tab == 3 {
                    let fv = self.repo_manager.filter.value().to_string();
                    self.repo_manager.profiles.handle_list_key(e.code, &mut self.repo_manager.filter_focus, &fv);
                } else if self.repo_manager.active_tab == 4 {
                    self.repo_manager.platforms.handle_list_key(e.code);
                } else {
                    *cursor_ref = cursor_ref.saturating_sub(page);
                }
            }
            KeyCode::PageDown => {
                if self.repo_manager.active_tab == 0 {
                    let n = self.repo_manager.group_order.len();
                    if n > 0 {
                        self.repo_manager.group_cursor = (self.repo_manager.group_cursor + 1) % n;
                        self.repo_manager.group_cursor_row = 0;
                    }
                } else if self.repo_manager.active_tab == 3 {
                    let fv = self.repo_manager.filter.value().to_string();
                    self.repo_manager.profiles.handle_list_key(e.code, &mut self.repo_manager.filter_focus, &fv);
                } else if self.repo_manager.active_tab == 4 {
                    self.repo_manager.platforms.handle_list_key(e.code);
                } else {
                    *cursor_ref = (*cursor_ref + page).min(max_cursor);
                }
            }
            KeyCode::Enter => {
                if self.repo_manager.active_tab == 4 {
                    // Platforms have no detail view
                } else if self.repo_manager.active_tab == 3 {
                    let name = match self.repo_manager.profiles.selected_profile_name() {
                        Some(n) => n.to_string(),
                        None => return true,
                    };
                    match self.load_profile_detail(&name) {
                        Ok((profile_models, modules, model_groups, ungrouped_modules, libraries)) => {
                            self.repo_manager.profiles.enter_detail(name, profile_models, modules, model_groups, ungrouped_modules, libraries);
                            self.status_at_profiles();
                        }
                        Err(e) => {
                            self.error_alert_visible = true;
                            self.error_alert_message = e;
                        }
                    }
                } else if self.repo_manager.active_tab == 2 && !self.repo_manager.model_rows.is_empty() {
                    self.repo_manager.info_visible = true;
                    self.repo_manager.info_row = self.repo_manager.model_cursor;
                    self.repo_manager.info_tab = 0;
                    self.repo_manager.info_scroll.set(0);
                    self.repo_manager.info_active_tab = 2;
                    self.status_at_repo_manager();
                } else if self.repo_manager.active_tab == 0 {
                    if self.repo_manager.group_cursor_row == 0 {
                        // Toggle expand/collapse on header
                        let gc = self.repo_manager.group_cursor;
                        if let Some(e) = self.repo_manager.group_expanded.get_mut(gc) {
                            *e = !*e;
                        }
                    } else if self.repo_manager.focused_module().is_some() {
                        self.repo_manager.info_visible = true;
                        self.repo_manager.info_row = self.repo_manager.group_cursor_row;
                        self.repo_manager.info_tab = 0;
                        self.repo_manager.info_scroll.set(0);
                        self.repo_manager.info_active_tab = 0;
                        self.status_at_repo_manager();
                    }
                } else if self.repo_manager.active_tab == 1 && !self.repo_manager.lib_rows.is_empty() {
                    self.repo_manager.info_visible = true;
                    self.repo_manager.info_row = self.repo_manager.lib_cursor;
                    self.repo_manager.info_tab = 0;
                    self.repo_manager.info_scroll.set(0);
                    self.repo_manager.info_active_tab = 1;
                    self.status_at_repo_manager();
                }
            }
            KeyCode::Delete => {
                if self.repo_manager.active_tab == 4 {
                    if let Some(name) = self.repo_manager.platforms.selected_name() {
                        self.repo_manager.platforms.open_delete(name);
                    }
                } else if self.repo_manager.active_tab == 3 {
                    if let Some(name) = self.repo_manager.profiles.selected_profile_name() {
                        self.repo_manager.profiles.open_delete(name.to_string());
                        self.status_at_profiles();
                    }
                } else if self.repo_manager.active_tab == 2 {
                    if let Some(model) = self.repo_manager.model_rows.get(self.repo_manager.model_cursor) {
                        self.repo_manager.open_model_delete(model.id.clone());
                    }
                } else if self.repo_manager.active_tab == 1 && !self.repo_manager.lib_rows.is_empty() {
                    self.repo_manager.delete_mode = true;
                    self.repo_manager.cross_platform_delete = false;
                    self.repo_manager.staged = self
                        .repo_manager
                        .lib_rows
                        .iter()
                        .map(|r| repomanager::StagedModule {
                            name: r.name.clone(),
                            version: Some(r.kind.clone()),
                            descr: r.checksum.clone(),
                            path: std::path::PathBuf::new(),
                            profile_modules: Vec::new(),
                            checked: false,
                            platform: None,
                            arch: None,
                        })
                        .collect();
                    self.repo_manager.staging_mode = repomanager::StagingMode::LibraryDelete;
                    self.repo_manager.staging = true;
                    self.repo_manager.staging_cursor = 0;
                    self.repo_manager.staging_focus = repomanager::StagingFocus::List;
                } else if self.repo_manager.active_tab == 0 {
                    let staged_rows: Option<Vec<repomanager::StagedModule>> = {
                        let rm = &self.repo_manager;
                        rm.focused_group_modules().map(|rows| {
                            rows.iter()
                                .map(|r| repomanager::StagedModule {
                                    name: r.name.clone(),
                                    version: r.version.clone(),
                                    descr: r.descr.clone(),
                                    path: std::path::PathBuf::new(),
                                    profile_modules: Vec::new(),
                                    checked: false,
                                    platform: Some(r.platform.clone()),
                                    arch: Some(r.arch.clone()),
                                })
                                .collect()
                        })
                    };
                    if let Some(rows) = staged_rows {
                        self.repo_manager.delete_mode = true;
                        self.repo_manager.cross_platform_delete = false;
                        self.repo_manager.staged = rows;
                        self.repo_manager.staging_mode = repomanager::StagingMode::ModuleDelete;
                        self.repo_manager.staging = true;
                        self.repo_manager.staging_cursor = 0;
                        self.repo_manager.staging_focus = repomanager::StagingFocus::List;
                    }
                }
            }
            KeyCode::Insert | KeyCode::Char('i') if !e.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.repo_manager.active_tab == 4 {
                    self.file_picker.open(&std::env::current_dir().unwrap_or_default(), filepicker::PickerMode::MinionBuild);
                } else if self.repo_manager.active_tab == 3 {
                    self.repo_manager.profiles.open_create();
                    self.status_at_profiles();
                } else if self.repo_manager.active_tab == 2 {
                    let start_dir = std::env::current_dir().unwrap_or_default();
                    self.file_picker.open(&start_dir, filepicker::PickerMode::DirectoryPicker);
                } else {
                    let mode = if self.repo_manager.active_tab == 1 { filepicker::PickerMode::LibrarySelector } else { filepicker::PickerMode::Any };
                    self.file_picker.open(&std::env::current_dir().unwrap_or_default(), mode);
                }
            }
            KeyCode::Char('l') if !e.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.repo_manager.active_tab == 3 {
                    self.repo_manager.profiles.open_create();
                    self.status_at_profiles();
                } else {
                    self.error_alert_visible = true;
                    self.error_alert_message = "Not implemented yet".to_string();
                }
            }
            KeyCode::Tab => {
                if self.repo_manager.active_tab == 0 {
                    let n = self.repo_manager.group_order.len().max(1);
                    let gc = self.repo_manager.group_cursor % n;
                    if let Some(e) = self.repo_manager.group_expanded.get_mut(gc) {
                        *e = !*e;
                    }
                } else {
                    self.repo_manager.filter_focus = true;
                }
            }
            KeyCode::Char('/') if !e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.repo_manager.filter_focus = true;
            }
            _ => {}
        }
        true
    }

    fn move_module_up(&mut self) {
        if self.repo_manager.group_cursor_row > 0 {
            self.repo_manager.group_cursor_row -= 1;
        } else {
            let n = self.repo_manager.group_order.len();
            if n == 0 {
                return;
            }
            self.repo_manager.group_cursor = (self.repo_manager.group_cursor + n - 1) % n;
            let gc = self.repo_manager.group_cursor;
            if self.repo_manager.group_expanded.get(gc).copied().unwrap_or(false) {
                if let Some(rows) = self.repo_manager.focused_group_modules() {
                    self.repo_manager.group_cursor_row = rows.len();
                } else {
                    self.repo_manager.group_cursor_row = 0;
                }
            } else {
                self.repo_manager.group_cursor_row = 0;
            }
        }
    }

    fn move_module_down(&mut self) {
        let n = self.repo_manager.group_order.len();
        if n == 0 {
            return;
        }
        let gc = self.repo_manager.group_cursor % n;
        if self.repo_manager.group_expanded.get(gc).copied().unwrap_or(false)
            && let Some(rows) = self.repo_manager.focused_group_modules()
            && self.repo_manager.group_cursor_row < rows.len()
        {
            self.repo_manager.group_cursor_row += 1;
        } else {
            self.repo_manager.group_cursor = (self.repo_manager.group_cursor + 1) % n;
            self.repo_manager.group_cursor_row = 0;
        }
    }

    fn repo_filtered_count(&self) -> usize {
        self.repo_manager.filtered_module_count(self.repo_manager.filter.value())
    }

    fn repo_filtered_lib_count(&self) -> usize {
        let f = self.repo_manager.filter.value().to_lowercase();
        self.repo_manager.lib_rows.iter().filter(|r| f.is_empty() || r.name.to_lowercase().contains(&f) || r.kind.to_lowercase().contains(&f)).count()
    }

    fn repo_filtered_model_count(&self) -> usize {
        let f = self.repo_manager.filter.value().to_lowercase();
        self.repo_manager.model_rows.iter().filter(|r| f.is_empty() || r.name.to_lowercase().contains(&f) || r.id.to_lowercase().contains(&f)).count()
    }

    fn call_profile_rpc(&self, context: &str) -> Result<ConsolePayload, String> {
        let ctx = context.to_string();
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}"), "*", None, None, Some(&ctx)).await })
        })
        .map_err(|e| format!("Profile RPC failed: {e}"))?;
        Ok(resp.payload)
    }

    fn load_profile_list(&mut self) -> Result<(), String> {
        let payload = self.call_profile_rpc(r#"{"op":"list"}"#)?;
        match payload {
            ConsolePayload::StringList { items } => {
                self.repo_manager.profiles.profiles = items;
                self.repo_manager.profiles.cursor = 0;
                Ok(())
            }
            _ => Err("Unexpected console payload for profile list".to_string()),
        }
    }

    fn load_profile_detail(&mut self, name: &str) -> Result<profiles::LoadedProfileDetail, String> {
        self.load_model_list()?;
        self.repo_manager.profiles.has_global_modules.set(self.repo_manager.module_groups.values().any(|v| !v.is_empty()));
        self.repo_manager.profiles.has_global_models.set(!self.repo_manager.model_rows.is_empty());
        let ctx_mods = serde_json::json!({"op": "list", "name": name, "library": false}).to_string();
        let payload_mods = self.call_profile_rpc(&ctx_mods)?;
        let module_selectors: Vec<String> = match payload_mods {
            ConsolePayload::StringList { items } => items,
            _ => return Err("Unexpected payload for profile module selectors".to_string()),
        };

        let ctx_libs = serde_json::json!({"op": "list", "name": name, "library": true}).to_string();
        let payload_libs = self.call_profile_rpc(&ctx_libs)?;
        let library_selectors: Vec<String> = match payload_libs {
            ConsolePayload::StringList { items } => items,
            _ => return Err("Unexpected payload for profile library selectors".to_string()),
        };

        let ctx_models = serde_json::json!({"op": "list", "name": name, "model": true}).to_string();
        let payload_models = self.call_profile_rpc(&ctx_models)?;
        let profile_models: Vec<String> = match payload_models {
            ConsolePayload::StringList { items } => items.iter().filter_map(|s| s.split_once(": ").map(|x| x.1.to_string())).collect(),
            _ => vec![],
        };

        let resolved_modules: Vec<profiles::ResolvedModule> = module_selectors
            .iter()
            .filter_map(|s| s.split_once(": ").map(|x| x.1))
            .flat_map(|sel| {
                self.repo_manager
                    .module_groups
                    .values()
                    .flatten()
                    .filter(|r| glob::Pattern::new(sel).is_ok_and(|p| p.matches(&r.name)))
                    .map(|r| profiles::ResolvedModule {
                        name: r.name.clone(),
                        version: r.version.clone().unwrap_or_default(),
                        descr: r.descr.clone(),
                        selector: sel.to_string(),
                        covered: true,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let resolved_libraries: Vec<profiles::ResolvedLibrary> = library_selectors
            .iter()
            .filter_map(|s| s.split_once(": ").map(|x| x.1))
            .flat_map(|sel| {
                self.repo_manager
                    .lib_rows
                    .iter()
                    .filter(|r| glob::Pattern::new(sel).is_ok_and(|p| p.matches(&r.name)))
                    .map(|r| profiles::ResolvedLibrary {
                        name: r.name.clone(),
                        kind: r.kind.clone(),
                        checksum: r.checksum.clone(),
                        selector: sel.to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let (model_groups, ungrouped_modules) = profiles::group_modules_by_models(&profile_models, &self.repo_manager.model_rows, &resolved_modules);

        Ok((profile_models, resolved_modules, model_groups, ungrouped_modules, resolved_libraries))
    }

    fn do_profile_create(&mut self, name: &str) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "new", "name": name}).to_string();
        self.call_profile_rpc(&ctx)?;
        self.load_profile_list()?;
        self.mark_repo_sync_pending();
        Ok(())
    }

    fn do_profile_delete(&mut self, name: &str) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "delete", "name": name}).to_string();
        self.call_profile_rpc(&ctx)?;
        self.load_profile_list()?;
        self.mark_repo_sync_pending();
        Ok(())
    }

    fn do_profile_add_matches(&mut self, name: &str, matches: Vec<String>, library: bool, model: bool) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "add", "name": name, "matches": matches, "library": library, "model": model}).to_string();
        self.call_profile_rpc(&ctx)?;
        self.mark_repo_sync_pending();
        Ok(())
    }

    fn do_profile_remove_match(&mut self, name: &str, selector: &str, library: bool) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "remove", "name": name, "matches": [selector], "library": library}).to_string();
        self.call_profile_rpc(&ctx)?;
        self.mark_repo_sync_pending();
        Ok(())
    }

    fn do_profile_tag(&mut self, profile_name: &str, minion_ids: &[String]) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "tag", "profiles": [profile_name]}).to_string();
        for mid in minion_ids {
            let ctx = ctx.clone();
            let mid = mid.clone();
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}"), "*", None, Some(&mid), Some(&ctx)).await
                })
            })
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn do_profile_untag(&mut self, profile_name: &str, minion_ids: &[String]) -> Result<(), String> {
        let ctx = serde_json::json!({"op": "untag", "profiles": [profile_name]}).to_string();
        for mid in minion_ids {
            let ctx = ctx.clone();
            let mid = mid.clone();
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}"), "*", None, Some(&mid), Some(&ctx)).await
                })
            })
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn bulk_add_profile_matches(&mut self, checked: Vec<repomanager::StagedModule>, library: bool, model: bool) {
        let names: Vec<String> = if model {
            checked.iter().map(|m| m.name.clone()).collect()
        } else if library {
            checked.iter().map(|m| m.name.clone()).collect()
        } else {
            let mut modules: Vec<String> =
                checked.iter().flat_map(|m| if m.profile_modules.is_empty() { vec![m.name.clone()] } else { m.profile_modules.clone() }).collect();
            modules.sort();
            modules.dedup();
            modules
        };
        let name = self.repo_manager.profiles.detail_name.clone();
        if let Err(e) = self.do_profile_add_matches(&name, names, library, model) {
            self.error_alert_visible = true;
            self.error_alert_message = e;
            return;
        }
        match self.load_profile_detail(&name) {
            Ok((profile_models, modules, model_groups, ungrouped_modules, libraries)) => {
                self.repo_manager.profiles.enter_detail(name, profile_models, modules, model_groups, ungrouped_modules, libraries);
            }
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = e;
            }
        }
    }

    fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
        let mut size = bytes as f64;
        let mut unit = 0;
        while size >= 1024.0 && unit < UNITS.len() - 1 {
            size /= 1024.0;
            unit += 1;
        }
        format!("{size:.1} {}", UNITS[unit])
    }

    fn load_platforms(&mut self) -> Result<(), String> {
        let repo_root = self.cfg.fileserver_root().join("repo");
        let repo = SysInspectModPak::new(repo_root).map_err(|e| format!("Cannot open repository: {e}"))?;
        let builds = repo.minion_builds();
        self.repo_manager.platforms.rows = builds
            .into_iter()
            .map(|r| {
                let chk = r.checksum().to_string();
                let size_str = std::fs::metadata(r.path()).ok().map(|m| Self::format_size(m.len())).unwrap_or_default();
                platforms::PlatformRow {
                    platform: r.platform().to_string(),
                    arch: r.arch().to_string(),
                    version: r.version().to_string(),
                    size: size_str,
                    checksum: if chk.len() > 12 { format!("{}…{}", &chk[..4], &chk[chk.len() - 4..]) } else { chk },
                }
            })
            .collect();
        self.repo_manager.platforms.cursor = 0;
        Ok(())
    }

    fn load_library_index(&mut self) -> Result<(), String> {
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_LIBRARY_INDEX}"), "*", None, None, None).await })
        })
        .map_err(|e| format!("Failed to get library index: {e}"))?;
        match resp.payload {
            ConsolePayload::MasterLibraryIndex { rows } => {
                self.repo_manager.lib_rows = rows;
                self.repo_manager.lib_cursor = 0;
                Ok(())
            }
            _ => Err("Unexpected console payload for library index".to_string()),
        }
    }

    fn load_model_list(&mut self) -> Result<(), String> {
        match self.get_models() {
            Ok((rows, _failures)) => {
                self.repo_manager.model_rows = rows;
                self.repo_manager.model_cursor = 0;
                Ok(())
            }
            Err(e) => Err(format!("Failed to load models: {e}")),
        }
    }

    fn model_dropin_path(&self) -> PathBuf {
        let cfg_path = self.cfg.config_path();
        let stem = cfg_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        cfg_path.with_file_name(format!("{stem}.d")).join("99-models.conf")
    }

    fn system_top_dropin_path(&self) -> PathBuf {
        let cfg_path = self.cfg.config_path();
        let stem = cfg_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        cfg_path.with_file_name(format!("{stem}.d")).join("system-top.conf")
    }

    fn write_system_top_dropin(&self) -> Result<(), String> {
        let dropin = self.system_top_dropin_path();
        if let Some(parent) = dropin.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Unable to create drop-in directory: {e}"))?;
        }
        let sort = match self.systop.persisted_sort() {
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Cpu => "cpu",
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Mem => "mem",
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Pid => "pid",
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Name => "name",
        };
        let graph = match self.systop.persisted_graph() {
            libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Blocks => "blocks",
            libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Line => "line",
        };
        let body = format!("config:\n  console:\n    tools:\n      system-top:\n        sort: {sort}\n        graph: {graph}\n");
        std::fs::write(&dropin, body).map_err(|e| format!("Unable to write System Top drop-in: {e}"))
    }

    fn persist_system_top_preferences(&self) {
        if let Err(err) = self.write_system_top_dropin() {
            log::warn!("Failed to persist System Top preferences: {err}");
        }
    }

    fn enabled_model_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.repo_manager.model_rows.iter().filter(|row| row.enabled).map(|row| row.id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    fn write_enabled_models_dropin(&self, mut ids: Vec<String>) -> Result<(), String> {
        ids.sort();
        ids.dedup();
        let dropin = self.model_dropin_path();
        if let Some(parent) = dropin.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Unable to create drop-in directory: {e}"))?;
        }
        let mut body = String::from("config:\n  master:\n    fileserver.models:");
        if ids.is_empty() {
            body.push_str(" []\n");
        } else {
            body.push('\n');
            for id in ids {
                body.push_str(&format!("      - {id}\n"));
            }
        }
        std::fs::write(&dropin, body).map_err(|e| format!("Unable to write models drop-in: {e}"))
    }

    fn refresh_local_model_rows(&mut self, enabled_ids: &[String]) -> Result<(), String> {
        let mut minion_cfg = MinionConfig::default();
        let root = self.cfg.fileserver_root().to_str().unwrap_or("/etc/sysinspect").to_string();
        minion_cfg.set_root_dir(&root);
        let enabled: std::collections::BTreeSet<String> = enabled_ids.iter().cloned().collect();
        let catalog = ModelCatalog::scan_root(Arc::new(minion_cfg), &self.cfg.fileserver_models_root(false));
        let rows: Vec<ConsoleModelRow> = catalog
            .successes()
            .into_iter()
            .map(|m| {
                let mut entrypoints: Vec<String> = Vec::new();
                let mut entrypoint_kinds: Vec<String> = Vec::new();
                let mut public_entrypoints: Vec<String> = Vec::new();
                let mut public_entrypoint_kinds: Vec<String> = Vec::new();
                #[allow(clippy::type_complexity)]
                let mut target_actions: Vec<(String, Vec<(String, Vec<String>, Vec<(String, String, bool)>)>)> = Vec::new();

                for ep in &m.entrypoints {
                    match ep {
                        libsysinspect::mdescr::browse_types::BrowsedEntrypoint::CheckbookLabel { label, entity_ids, .. } => {
                            entrypoints.push(label.clone());
                            entrypoint_kinds.push("checkbook".to_string());
                            #[allow(clippy::type_complexity)]
                            let actions: Vec<(String, Vec<String>, Vec<(String, String, bool)>)> = m
                                .actions
                                .iter()
                                .filter(|a| a.binds_to.iter().any(|eid| entity_ids.contains(eid)))
                                .map(|a| {
                                    let states: Vec<String> = a.states.iter().map(|s| s.state.clone()).collect();
                                    let ctx_vars: Vec<(String, String, bool)> = a.states.iter().flat_map(|s| s.context_vars.clone()).collect();
                                    (a.description.clone(), states, ctx_vars)
                                })
                                .collect();
                            target_actions.push((label.clone(), actions));
                        }
                        libsysinspect::mdescr::browse_types::BrowsedEntrypoint::Entity { id, .. } => {
                            entrypoints.push(id.clone());
                            entrypoint_kinds.push("entity".to_string());
                            #[allow(clippy::type_complexity)]
                            let actions: Vec<(String, Vec<String>, Vec<(String, String, bool)>)> = m
                                .actions
                                .iter()
                                .filter(|a| a.binds_to.contains(id))
                                .map(|a| {
                                    let states: Vec<String> = a.states.iter().map(|s| s.state.clone()).collect();
                                    let ctx_vars: Vec<(String, String, bool)> = a.states.iter().flat_map(|s| s.context_vars.clone()).collect();
                                    (a.description.clone(), states, ctx_vars)
                                })
                                .collect();
                            target_actions.push((id.clone(), actions));
                        }
                    }
                }

                for ep in &m.public_entrypoints {
                    match ep {
                        libsysinspect::mdescr::browse_types::BrowsedEntrypoint::CheckbookLabel { label, .. } => {
                            public_entrypoints.push(label.clone());
                            public_entrypoint_kinds.push("checkbook".to_string());
                        }
                        libsysinspect::mdescr::browse_types::BrowsedEntrypoint::Entity { id, .. } => {
                            public_entrypoints.push(id.clone());
                            public_entrypoint_kinds.push("entity".to_string());
                        }
                    }
                }

                ConsoleModelRow {
                    id: m.metadata.id.clone(),
                    enabled: enabled.contains(&m.metadata.id),
                    name: m.metadata.name.clone(),
                    version: m.metadata.version.clone(),
                    description: m.metadata.description.clone(),
                    entrypoints,
                    entrypoint_kinds,
                    public_entrypoints,
                    public_entrypoint_kinds,
                    public_actions: m.public_actions.clone(),
                    modules: m.modules.clone(),
                    states: m.states.clone(),
                    target_actions,
                }
            })
            .collect();
        let cursor = self.repo_manager.model_cursor.min(rows.len().saturating_sub(1));
        self.repo_manager.model_rows = rows;
        self.repo_manager.model_cursor = cursor;
        Ok(())
    }

    fn reload_master_config(&self) -> Result<(), String> {
        let resp = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_CONFIG_RELOAD}"), "*", None, None, None).await })
        })
        .map_err(|e| format!("Failed to reload master config: {e}"))?;
        if resp.ok { Ok(()) } else { Err(if resp.error.is_empty() { "Master config reload failed".to_string() } else { resp.error }) }
    }

    fn set_model_enabled(&mut self, model_id: &str, enabled: bool) -> Result<(), String> {
        let mut ids = self.enabled_model_ids();
        if enabled {
            if !ids.iter().any(|id| id == model_id) {
                ids.push(model_id.to_string());
            }
        } else {
            ids.retain(|id| id != model_id);
        }
        self.write_enabled_models_dropin(ids)?;
        self.reload_master_config()?;
        self.refresh_local_model_rows(&self.enabled_model_ids_with(model_id, enabled))?;
        self.repo_manager.models_dirty = true;
        Ok(())
    }

    fn enabled_model_ids_with(&self, model_id: &str, enabled: bool) -> Vec<String> {
        let mut ids = self.enabled_model_ids();
        if enabled {
            if !ids.iter().any(|id| id == model_id) {
                ids.push(model_id.to_string());
            }
        } else {
            ids.retain(|id| id != model_id);
        }
        ids.sort();
        ids.dedup();
        ids
    }

    fn process_model_add(&mut self, path: &std::path::Path) {
        if !path.is_dir() {
            self.error_alert_visible = true;
            self.error_alert_message = "Select a model directory".to_string();
            return;
        }
        if !path.join("model.cfg").exists() {
            self.error_alert_visible = true;
            self.error_alert_message = "Selected directory does not contain model.cfg".to_string();
            return;
        }
        let model_id = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if model_id.is_empty() {
            self.error_alert_visible = true;
            self.error_alert_message = "Unable to determine model id from directory name".to_string();
            return;
        }
        let dst_root = self.cfg.fileserver_models_root(false);
        let dst = dst_root.join(&model_id);
        if dst.exists() {
            self.error_alert_visible = true;
            self.error_alert_message = format!("Model already exists: {model_id}");
            return;
        }
        let enabled_ids = self.enabled_model_ids_with(&model_id, true);
        match Self::copy_dir_recursive(path, &dst)
            .and_then(|_| self.write_enabled_models_dropin(enabled_ids.clone()))
            .and_then(|_| self.reload_master_config())
            .and_then(|_| self.refresh_local_model_rows(&enabled_ids))
        {
            Ok(()) => {
                self.repo_manager.models_dirty = true;
                self.info_alert_visible = true;
                self.info_alert_title = "Model Import".to_string();
                self.info_alert_styled = None;
                self.info_alert_message = format!("Model added: {model_id}");
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&dst);
                self.error_alert_visible = true;
                self.error_alert_message = err;
            }
        }
    }

    fn delete_model(&mut self, model_id: &str) -> Result<(), String> {
        let path = self.cfg.fileserver_models_root(false).join(model_id);
        if !path.exists() {
            return Err(format!("Model does not exist: {model_id}"));
        }
        std::fs::remove_dir_all(&path).map_err(|e| format!("Unable to remove model {model_id}: {e}"))?;
        let enabled_ids = self.enabled_model_ids_with(model_id, false);
        self.write_enabled_models_dropin(enabled_ids.clone())?;
        self.refresh_local_model_rows(&enabled_ids)?;
        self.repo_manager.models_dirty = true;
        Ok(())
    }

    fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
        std::fs::create_dir_all(dst).map_err(|e| format!("Unable to create destination {}: {e}", dst.display()))?;
        let entries = std::fs::read_dir(src).map_err(|e| format!("Unable to read {}: {e}", src.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Unable to read directory entry in {}: {e}", src.display()))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path).map_err(|e| format!("Unable to copy {} to {}: {e}", src_path.display(), dst_path.display()))?;
            }
        }
        Ok(())
    }

    fn process_module_add(&mut self, path: &std::path::Path) {
        if path.is_dir() {
            let mut staged = Self::scan_dir_for_modules(path);
            if staged.is_empty() {
                self.error_alert_visible = true;
                self.error_alert_message = "No .spec files found in the selected directory".to_string();
            } else {
                let total = staged.len();
                let flat_modules: Vec<ConsoleModuleRow> = self.repo_manager.module_groups.values().flatten().cloned().collect();
                Self::dedup_staged_modules(&flat_modules, &mut staged);
                let skipped = total - staged.len();
                if staged.is_empty() {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("No new modules found, {skipped} skipped");
                } else {
                    self.repo_manager.enter_staging(staged);
                }
            }
        } else {
            let spec = path.with_extension("spec");
            if spec.exists() {
                let module_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let (version, descr) = Self::read_spec_version_descr(&spec);
                self.repo_manager.enter_staging(vec![repomanager::StagedModule {
                    name: module_name,
                    version,
                    descr,
                    path: path.to_path_buf(),
                    profile_modules: Vec::new(),
                    checked: true,
                    platform: None,
                    arch: None,
                }]);
            } else {
                self.error_alert_visible = true;
                self.error_alert_message = "Module has no specfile. Add this module manually via CLI.".to_string();
            }
        }
    }

    fn scan_dir_for_modules(root: &std::path::Path) -> Vec<repomanager::StagedModule> {
        let mut staged = Vec::new();
        let Ok(entries) = std::fs::read_dir(root) else { return staged };
        for entry in entries.flatten() {
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let dir_name = sub.file_name().unwrap_or_default().to_string_lossy().to_string();
            let spec = sub.join(format!("{dir_name}.spec"));
            if !spec.exists() {
                continue;
            }
            let module_name = Self::read_spec_name(&spec).unwrap_or_else(|| dir_name.clone());
            let (version, descr) = Self::read_spec_version_descr(&spec);
            let bin = sub.join(&dir_name);
            staged.push(repomanager::StagedModule {
                name: module_name,
                version,
                descr,
                path: if bin.exists() { bin } else { spec },
                profile_modules: Vec::new(),
                checked: true,
                platform: None,
                arch: None,
            });
        }
        staged
    }

    fn read_spec_name(spec: &std::path::Path) -> Option<String> {
        match std::fs::read_to_string(spec) {
            Ok(yaml) => match serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
                Ok(v) => v.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                Err(_) => None,
            },
            Err(_) => None,
        }
    }

    fn dedup_staged_modules(existing: &[ConsoleModuleRow], staged: &mut Vec<repomanager::StagedModule>) {
        staged.retain(|m| !existing.iter().any(|r| r.name == m.name && r.version == m.version));
    }

    fn bulk_add_modules(&mut self, staged: Vec<repomanager::StagedModule>) {
        let total = staged.len();
        *self.repo_manager.progress.lock().unwrap() = Some((0, total));
        let progress = self.repo_manager.progress.clone();
        let repo_root = self.cfg.fileserver_root().join("repo");

        std::thread::spawn(move || {
            let mut repo = match SysInspectModPak::new(repo_root) {
                Ok(r) => r,
                Err(e) => {
                    *progress.lock().unwrap() = None;
                    // Can't access self.error_alert here — just log
                    log::error!("Cannot open repository: {e}");
                    return;
                }
            };
            for (i, m) in staged.iter().enumerate() {
                let spec_path = m.path.with_extension("spec");
                let spec_yaml = match std::fs::read_to_string(&spec_path) {
                    Ok(y) => y,
                    Err(e) => {
                        log::error!("Cannot read spec {}: {e}", m.name);
                        *progress.lock().unwrap() = None;
                        return;
                    }
                };
                let mi: ModInterface = match serde_yaml::from_str(&spec_yaml) {
                    Ok(mi) => mi,
                    Err(e) => {
                        log::error!("Invalid spec {}: {e}", m.name);
                        *progress.lock().unwrap() = None;
                        return;
                    }
                };
                let meta = match ModPakMetadata::from_spec(&mi, m.path.clone()) {
                    Ok(meta) => meta,
                    Err(e) => {
                        log::error!("Invalid spec data {}: {e}", m.name);
                        *progress.lock().unwrap() = None;
                        return;
                    }
                };
                if let Err(e) = repo.add_module(meta) {
                    log::error!("Cannot add module {}: {e}", m.name);
                    *progress.lock().unwrap() = None;
                    return;
                }
                *progress.lock().unwrap() = Some((i + 1, total));
            }
            *progress.lock().unwrap() = None;
        });
    }

    fn bulk_delete_modules(&mut self, names: &[String]) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        let mut repo = match SysInspectModPak::new(repo_root) {
            Ok(r) => r,
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
                return;
            }
        };
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        if let Err(e) = repo.remove_module(name_refs) {
            self.error_alert_visible = true;
            self.error_alert_message = format!("Cannot remove modules: {e}");
        } else {
            let _ = self.load_module_index();
            self.mark_repo_sync_pending();
        }
    }

    fn bulk_delete_single_platform(&mut self, checked: &[repomanager::StagedModule]) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        let mut repo = match SysInspectModPak::new(repo_root) {
            Ok(r) => r,
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
                return;
            }
        };
        for m in checked {
            if let (Some(platform), Some(arch)) = (m.platform.as_ref(), m.arch.as_ref())
                && let Err(e) = repo.remove_module_single(&m.name, platform, arch)
            {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot remove module: {e}");
                return;
            }
        }
        let _ = self.load_module_index();
        self.mark_repo_sync_pending();
    }

    fn bulk_delete_libraries(&mut self, names: &[String]) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        let mut repo = match SysInspectModPak::new(repo_root) {
            Ok(r) => r,
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
                return;
            }
        };
        if let Err(e) = repo.remove_library(names.to_vec()) {
            self.error_alert_visible = true;
            self.error_alert_message = format!("Cannot remove libraries: {e}");
        } else {
            let _ = self.load_library_index();
            self.mark_repo_sync_pending();
        }
    }

    fn process_library_add(&mut self, path: &std::path::Path) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        let mut repo = match SysInspectModPak::new(repo_root) {
            Ok(r) => r,
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
                return;
            }
        };
        if path.is_dir() {
            if let Err(e) = repo.add_library(path.to_path_buf()) {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot add library: {e}");
            } else {
                self.load_library_index().ok();
                self.mark_repo_sync_pending();
            }
        } else {
            // Single file: wrap in temp dir, use add_library
            let tmp = std::env::temp_dir().join("sysinspect_lib_add");
            let _ = std::fs::create_dir_all(&tmp);
            let dest = tmp.join(path.file_name().unwrap_or_default());
            match std::fs::copy(path, &dest) {
                Ok(_) => {
                    if let Err(e) = repo.add_library(tmp.clone()) {
                        self.error_alert_visible = true;
                        self.error_alert_message = format!("Cannot add library file: {e}");
                    } else {
                        self.load_library_index().ok();
                        self.mark_repo_sync_pending();
                    }
                    let _ = std::fs::remove_dir_all(&tmp);
                }
                Err(e) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Cannot copy library file: {e}");
                }
            }
        }
    }

    fn process_platform_add(&mut self, path: &std::path::Path) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        match SysInspectModPak::new(repo_root) {
            Ok(mut repo) => {
                if let Err(e) = repo.add_minion_build(path.to_path_buf()) {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Cannot add minion build: {e}");
                } else if let Err(e) = self.load_platforms() {
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Failed to reload platforms: {e}");
                } else {
                    self.mark_cluster_upgrade_required();
                }
            }
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
            }
        }
    }

    fn do_platform_remove(&mut self, name: &str) {
        let repo_root = self.cfg.fileserver_root().join("repo");
        match SysInspectModPak::new(repo_root) {
            Ok(mut repo) => {
                let _ = repo.remove_minion_build(vec![name.to_string()]);
            }
            Err(e) => {
                self.error_alert_visible = true;
                self.error_alert_message = format!("Cannot open repository: {e}");
                return;
            }
        }
        let _ = self.load_platforms();
        self.mark_cluster_upgrade_required();
    }

    fn read_spec_version_descr(spec: &std::path::Path) -> (Option<String>, String) {
        match std::fs::read_to_string(spec) {
            Ok(yaml) => match serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
                Ok(v) => (
                    v.get("version").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    v.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
                ),
                Err(_) => (None, String::new()),
            },
            Err(_) => (None, String::new()),
        }
    }

    fn on_master_logs_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.master_logs_visible {
            return false;
        }
        let page = self.master_logs_viewport_rows.get().max(1);
        let section = match self.master_logs_sections.get(self.master_logs_tab) {
            Some(s) => s,
            None => return true,
        };
        let rendered = Self::filtered_master_rendered_lines(&section.lines, &self.master_logs_filter);
        let total_rows = rendered.len();
        let max_top = total_rows.saturating_sub(page);

        if self.master_logs_filter_focus {
            match e.code {
                KeyCode::Esc => {
                    self.master_logs_filter_focus = false;
                }
                KeyCode::Tab => {
                    self.master_logs_filter_focus = false;
                }
                KeyCode::Backspace => {
                    self.master_logs_filter.delete_before();
                }
                KeyCode::Delete => {
                    self.master_logs_filter.delete_at();
                }
                KeyCode::Left => {
                    self.master_logs_filter.move_left();
                }
                KeyCode::Right => {
                    self.master_logs_filter.move_right();
                }
                KeyCode::Home => {
                    self.master_logs_filter.home();
                }
                KeyCode::End => {
                    self.master_logs_filter.end();
                }
                KeyCode::Char(c) => {
                    self.master_logs_filter.insert_char(c);
                }
                _ => {}
            }
            return true;
        }

        match e.code {
            KeyCode::Esc => {
                self.master_logs_visible = false;
            }
            KeyCode::Left => {
                self.master_logs_tab = self.master_logs_tab.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.master_logs_tab + 1 < self.master_logs_sections.len() {
                    self.master_logs_tab += 1;
                }
            }
            KeyCode::Tab => {
                self.master_logs_filter_focus = true;
            }
            KeyCode::Up => {
                let s = &self.master_logs_sections[self.master_logs_tab];
                let mut scroll = s.scroll.get();
                if scroll == usize::MAX {
                    scroll = max_top;
                }
                scroll = scroll.saturating_sub(1);
                s.scroll.set(scroll);
            }
            KeyCode::Down => {
                let s = &self.master_logs_sections[self.master_logs_tab];
                let mut scroll = s.scroll.get();
                if scroll == usize::MAX {
                    return true;
                }
                scroll = (scroll + 1).min(max_top);
                if scroll >= max_top {
                    scroll = usize::MAX;
                }
                s.scroll.set(scroll);
            }
            KeyCode::PageUp => {
                let s = &self.master_logs_sections[self.master_logs_tab];
                let mut scroll = s.scroll.get();
                if scroll == usize::MAX {
                    scroll = max_top;
                }
                scroll = scroll.saturating_sub(page);
                s.scroll.set(scroll);
            }
            KeyCode::PageDown => {
                let s = &self.master_logs_sections[self.master_logs_tab];
                let mut scroll = s.scroll.get();
                if scroll == usize::MAX {
                    return true;
                }
                scroll = (scroll + page).min(max_top);
                if scroll >= max_top {
                    scroll = usize::MAX;
                }
                s.scroll.set(scroll);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Err(err) = self.load_master_logs() {
                    self.error_alert_visible = true;
                    self.error_alert_message = err.to_string();
                }
            }
            KeyCode::Char('/') => {
                self.master_logs_filter_focus = true;
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.master_logs_polling = !self.master_logs_polling;
                self.status_at_master_logs();
            }
            _ => {}
        }
        true
    }

    fn find_sysmaster_binary(&self) -> Option<PathBuf> {
        // 1. Self-contained: {root}/bin/sysmaster
        let root = self.cfg.root_dir();
        let candidate = root.join("bin/sysmaster");
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
        // 2. Same dir as current binary
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let candidate = dir.join("sysmaster");
            if candidate.exists() && candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn on_master_menu(&mut self, e: event::KeyEvent) -> bool {
        if !self.master_menu_visible {
            return false;
        }
        match e.code {
            KeyCode::Esc => {
                self.master_menu_visible = false;
                self.status_at_cycles();
            }
            KeyCode::Char('o') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                if self.evtipc.is_some() {
                    self.open_master_logs();
                } else {
                    self.error_alert_visible = true;
                    self.error_alert_message = "Master is not running".to_string();
                }
            }
            KeyCode::Char('l') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                match self.load_master_logs_local() {
                    Ok(()) => {
                        self.master_logs_visible = true;
                        self.master_logs_tab = 0;
                        self.master_logs_polling = false;
                        self.status_at_master_logs();
                    }
                    Err(e) => {
                        self.error_alert_visible = true;
                        self.error_alert_message = e;
                    }
                }
            }
            KeyCode::Char('r') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                self.registration_form.visible = true;
            }
            KeyCode::Char('a') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                if let Err(err) = self.load_module_index() {
                    self.error_alert_visible = true;
                    self.error_alert_message = err;
                } else {
                    self.repo_manager.visible = true;
                    self.status_at_repo_manager();
                }
            }
            KeyCode::Char('u') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                self.start_cluster_upgrade();
            }
            KeyCode::Char('t') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 1;
            }
            KeyCode::Char('s') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 3;
            }
            KeyCode::Char('e') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = false;
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 2;
            }
            KeyCode::Up | KeyCode::PageUp => {
                let total = macts::total_master_menu_items();
                if total > 0 {
                    self.master_menu_sel = if self.master_menu_sel == 0 { total - 1 } else { self.master_menu_sel - 1 };
                }
            }
            KeyCode::Down | KeyCode::PageDown => {
                let total = macts::total_master_menu_items();
                if total > 0 {
                    self.master_menu_sel = (self.master_menu_sel + 1) % total;
                }
            }
            KeyCode::Enter => {
                self.master_menu_visible = false;
                match self.master_menu_sel {
                    0 => {
                        if self.evtipc.is_some() {
                            self.open_master_logs();
                        } else {
                            self.error_alert_visible = true;
                            self.error_alert_message = "Master is not running".to_string();
                        }
                    }
                    1 => match self.load_master_logs_local() {
                        Ok(()) => {
                            self.master_logs_visible = true;
                            self.master_logs_tab = 0;
                            self.master_logs_polling = false;
                            self.status_at_master_logs();
                        }
                        Err(e) => {
                            self.error_alert_visible = true;
                            self.error_alert_message = e;
                        }
                    },
                    2 => {
                        self.registration_form.visible = true;
                    }
                    3 => {
                        if let Err(err) = self.load_module_index() {
                            self.error_alert_visible = true;
                            self.error_alert_message = err;
                        } else {
                            self.repo_manager.visible = true;
                            self.status_at_repo_manager();
                        }
                    }
                    4 => {
                        self.start_cluster_upgrade();
                    }
                    5 => {
                        self.master_confirm_visible = true;
                        self.master_confirm_choice = AlertResult::Default;
                        self.master_confirm_action = 1;
                    }
                    6 => {
                        self.master_confirm_visible = true;
                        self.master_confirm_choice = AlertResult::Default;
                        self.master_confirm_action = 3;
                    }
                    7 => {
                        self.master_confirm_visible = true;
                        self.master_confirm_choice = AlertResult::Default;
                        self.master_confirm_action = 2;
                    }
                    _ => {}
                }
                self.status_at_cycles();
            }
            _ => {}
        }
        true
    }

    fn on_master_confirm(&mut self, e: event::KeyEvent) -> bool {
        if !self.master_confirm_visible {
            return false;
        }
        match e.code {
            KeyCode::Tab => {
                self.master_confirm_choice =
                    if self.master_confirm_choice == AlertResult::Default { AlertResult::Quit } else { AlertResult::Default };
            }
            KeyCode::Esc => {
                self.master_confirm_visible = false;
                self.master_confirm_action = 0;
            }
            KeyCode::Enter => {
                self.master_confirm_visible = false;
                let action = self.master_confirm_action;
                self.master_confirm_action = 0;
                if self.master_confirm_choice == AlertResult::Quit {
                    match action {
                        1 => self.do_master_start(),
                        2 => self.do_master_restart(),
                        3 => self.do_master_stop(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn do_master_start(&mut self) {
        if let Some(bin) = self.find_sysmaster_binary() {
            let config_path = {
                let root = self.cfg.root_dir();
                let etc_path = root.join("etc/sysinspect.conf");
                if etc_path.exists() { etc_path } else { root.join("sysinspect.conf") }
            };
            let child = std::process::Command::new(&bin)
                .arg("--daemon")
                .arg("-c")
                .arg(config_path.to_string_lossy().as_ref())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Ok(c) = child {
                std::thread::spawn(move || {
                    c.wait_with_output().ok();
                });
            }
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if self.try_reconnect_silent().is_ok() {
                    return;
                }
            }
            self.error_alert_visible = true;
            self.error_alert_message = "Master started but not reachable yet.".to_string();
        } else {
            self.error_alert_visible = true;
            self.error_alert_message = "Cannot find sysmaster binary".to_string();
        }
    }

    fn do_master_stop(&mut self) {
        if let Err(err) = libsysinspect::util::sys::kill_process(self.cfg.pidfile(), None) {
            self.error_alert_visible = true;
            self.error_alert_message = format!("Failed to stop master: {err}");
        } else {
            self.offline = true;
            self.evtipc = None;
        }
    }

    fn do_master_restart(&mut self) {
        self.do_master_stop();
        std::thread::sleep(std::time::Duration::from_secs(2));
        self.do_master_start();
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
                self.li_events = Vec::new();
                self.li_minions = Vec::new();
                self.selected_event = 0;

                if down {
                    if self.selected_cycle < self.cycles_buf.len().saturating_sub(1) {
                        self.selected_cycle += 1;
                    }
                } else if self.selected_cycle > 0 {
                    self.selected_cycle -= 1;
                }
                if !self.cycles_buf.is_empty() {
                    let sid = self.get_selected_cycle().event().sid().to_string();
                    if let Ok(minions) = self.get_minions(&sid) {
                        self.li_minions = minions;
                        self.selected_minion = 0;
                        self.refresh_events_for_selected_minion();
                    }
                }
            }
            Err(err) => {
                self.error_alert_visible = true;
                self.error_alert_message = err.to_string();
            }
        }
    }

    fn refresh_events_for_selected_minion(&mut self) {
        self.event_data = IndexMap::new();
        self.li_events = Vec::new();
        self.selected_event = 0;
        if let Some(mli) = self.get_selected_minion() {
            let sid = self.get_selected_cycle().event().sid().to_string();
            if let Ok(events) = self.get_events(&sid, mli.event().id()) {
                self.li_events = events;
            }
        }
        if !self.li_events.is_empty() {
            self.event_data = self.li_events[0].event().flatten();
        }
    }

    fn refresh_selected_cycle_contents_preserve_selection(&mut self) {
        if self.cycles_buf.is_empty() {
            self.li_minions.clear();
            self.li_events.clear();
            self.event_data.clear();
            self.selected_minion = 0;
            self.selected_event = 0;
            return;
        }

        let sid = self.get_selected_cycle().event().sid().to_string();
        let selected_mid = self.get_selected_minion().map(|mli| mli.event().id().to_string());
        let selected_event = self.get_selected_event().map(|evt| {
            let event = evt.event();
            (event.get_action_id(), event.get_entity_id(), event.get_status_id(), event.get_timestamp())
        });

        match self.get_minions(&sid) {
            Ok(minions) => {
                self.li_minions = minions;
                if let Some(selected_mid) = selected_mid
                    && let Some(idx) = self.li_minions.iter().position(|mli| mli.event().id() == selected_mid)
                {
                    self.selected_minion = idx;
                } else if self.selected_minion >= self.li_minions.len() {
                    self.selected_minion = self.li_minions.len().saturating_sub(1);
                }
            }
            Err(_) => return,
        }

        let Some(mli) = self.get_selected_minion() else {
            self.li_events.clear();
            self.event_data.clear();
            self.selected_event = 0;
            return;
        };

        match self.get_events(&sid, mli.event().id()) {
            Ok(events) => {
                self.li_events = events;
                if let Some((aid, eid, sid, ts)) = selected_event
                    && let Some(idx) = self.li_events.iter().position(|evt| {
                        let event = evt.event();
                        event.get_action_id() == aid && event.get_entity_id() == eid && event.get_status_id() == sid && event.get_timestamp() == ts
                    })
                {
                    self.selected_event = idx;
                } else if self.selected_event >= self.li_events.len() {
                    self.selected_event = self.li_events.len().saturating_sub(1);
                }
            }
            Err(_) => return,
        }

        if let Some(evt) = self.get_selected_event() {
            self.event_data = evt.event().flatten();
        } else {
            self.event_data.clear();
        }
    }

    fn on_mouse_move(&mut self, me: MouseEvent) {
        let rects = match self.popup_button_rects.get() {
            Some(r) => r,
            None => return,
        };
        let (cx, cy) = (me.column, me.row);
        let hit = |r: Rect| cx >= r.x && cx < r.x.saturating_add(r.width) && cy == r.y;
        let on_left = rects.left_button.is_some_and(hit);
        let on_right = hit(rects.right_button);

        if self.error_alert_visible || self.info_alert_visible || self.help_popup_visible {
            return;
        }
        if self.exit_alert_visible {
            if on_left {
                self.exit_alert_choice = AlertResult::Quit;
            } else if on_right {
                self.exit_alert_choice = AlertResult::Default;
            }
            return;
        }
        if self.purge_alert_visible {
            if on_left {
                self.purge_alert_choice = AlertResult::Purge;
            } else if on_right {
                self.purge_alert_choice = AlertResult::Default;
            }
            return;
        }
        if self.cluster_confirm_visible {
            if self.pending_cluster_action == 3 {
                if on_left {
                    self.cluster_confirm_form_focus = DialogFormFocus::LeftButton;
                } else if on_right {
                    self.cluster_confirm_form_focus = DialogFormFocus::RightButton;
                }
            } else if on_left {
                self.cluster_confirm_choice = AlertResult::ClusterConfirm;
            } else if on_right {
                self.cluster_confirm_choice = AlertResult::Default;
            }
            return;
        }
        if self.master_confirm_visible {
            if on_left {
                self.master_confirm_choice = AlertResult::Quit;
            } else if on_right {
                self.master_confirm_choice = AlertResult::Default;
            }
        }
    }

    fn on_mouse_click(&mut self, me: MouseEvent) {
        let rects = match self.popup_button_rects.get() {
            Some(r) => r,
            None => return,
        };
        let (cx, cy) = (me.column, me.row);
        let hit = |r: Rect| cx >= r.x && cx < r.x.saturating_add(r.width) && cy == r.y;
        let on_left = rects.left_button.is_some_and(hit);
        let on_right = hit(rects.right_button);

        if self.error_alert_visible {
            self.error_alert_visible = false;
            return;
        }
        if self.info_alert_visible {
            self.info_alert_visible = false;
            return;
        }
        if self.help_popup_visible {
            self.help_popup_visible = false;
            return;
        }
        if self.exit_alert_visible {
            if on_left {
                self.exit_alert_choice = AlertResult::Quit;
            } else if on_right {
                self.exit_alert_choice = AlertResult::Default;
            } else {
                return;
            }
            self.exit_alert_visible = false;
            if self.exit_alert_choice == AlertResult::Quit {
                self.exit = true;
            }
            return;
        }
        if self.purge_alert_visible {
            if !on_left && !on_right {
                return;
            }
            if on_left {
                self.purge_alert_choice = AlertResult::Purge;
                let _ = self.purge_database();
            }
            self.purge_alert_visible = false;
            self.status_text = Line::from(Span::styled("", Style::default().fg(palette::FG)));
            return;
        }
        if self.cluster_confirm_visible {
            if self.pending_cluster_action == 3 {
                if on_left {
                    let force = self.delete_force_remove;
                    self.close_cluster_confirm();
                    self.do_minion_delete(force);
                } else if on_right {
                    self.close_cluster_confirm();
                }
            } else if on_left {
                self.cluster_confirm_choice = AlertResult::ClusterConfirm;
                self.cluster_confirm_visible = false;
                match self.pending_cluster_action {
                    1 => self.do_cluster_shutdown(),
                    2 => self.do_cluster_reconnect(),
                    _ => {}
                }
                self.pending_cluster_action = 0;
                self.status_at_minions_browser();
            } else if on_right {
                self.close_cluster_confirm();
            }
            return;
        }
        if self.master_confirm_visible {
            if on_left {
                self.master_confirm_choice = AlertResult::Quit;
                self.master_confirm_visible = false;
                match self.master_confirm_action {
                    1 => self.do_master_start(),
                    2 => self.do_master_restart(),
                    3 => self.do_master_stop(),
                    _ => {}
                }
            } else if on_right {
                self.master_confirm_visible = false;
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

        if self.delete_progress.visible {
            return;
        }

        if self.cluster_upgrade_progress.visible {
            return;
        }

        if self.cluster_start_progress.visible {
            return;
        }

        // Master operations menu is modal
        if self.on_master_menu(e) {
            return;
        }

        // Exit alert takes priority over setup wizard
        if self.on_exit_alert(e) {
            return;
        }

        // Setup wizard is modal (but not when file picker is open)
        if self.setup_wizard.visible && !self.file_picker.visible {
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

        // Registration form is modal
        if self.registration_form.visible {
            self.status_at_registration_form();
            self.registration_form.handle_key(e);
            if self.registration_form.ok_pressed {
                self.registration_form.ok_pressed = false;
                self.registration_form.visible = false;
                self.status_at_registration_progress();
                let hostname = self.registration_form.hostname.value().to_string();
                let user = self.registration_form.user.value().to_string();
                let path = self.registration_form.path.value().to_string();
                let use_sudo = self.registration_form.use_sudo;
                let progress = Arc::new(std::sync::Mutex::new(minreg::RegistrationProgress::new(String::new())));
                self.registration_progress = Arc::clone(&progress);
                self.registration_task = Some(minreg::spawn_registration(hostname, user, path, use_sudo, self.cfg.clone(), progress));
            }
            return;
        }

        // Registration progress is modal
        let progress_visible = self.registration_progress.lock().unwrap().visible;
        if progress_visible {
            self.status_at_registration_progress();
            let mut progress = self.registration_progress.lock().unwrap();
            if minreg::handle_progress_key(e, &mut progress) {
                if progress.error.is_some() || progress.done {
                    drop(progress);
                    *self.registration_progress.lock().unwrap() = minreg::RegistrationProgress::placeholder();
                    self.restore_status();
                }
                return;
            }
            return;
        }
        if self.file_picker.visible && self.file_picker.handle_key(e) {
            return;
        }

        // Repo manager is modal
        if self.on_repo_manager(e) {
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
                    let _target = self
                        .dsl_browser
                        .target_entities
                        .items
                        .get(self.dsl_browser.target_entities.selected().unwrap_or(0))
                        .map(|s| s.as_str())
                        .unwrap_or("");
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

        if self.on_master_confirm(e) {
            return;
        }

        if self.on_error_alert(e) {
            return;
        }

        if self.on_tag_popup(e) {
            return;
        }

        if self.on_master_logs_popup(e) {
            return;
        }

        if self.on_minions_popup(e) {
            return;
        }

        match e.code {
            KeyCode::PageUp => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        self.event_data = IndexMap::new();
                        self.li_events = Vec::new();
                        self.li_minions = Vec::new();
                        self.selected_event = 0;
                        self.selected_cycle = self.selected_cycle.saturating_sub(self.size.get().table_cycles);
                        if !self.cycles_buf.is_empty() {
                            let sid = self.get_selected_cycle().event().sid().to_string();
                            if let Ok(minions) = self.get_minions(&sid) {
                                self.li_minions = minions;
                                self.selected_minion = 0;
                            }
                        }
                    }
                    ActiveBox::Minions => {
                        self.selected_minion = self.selected_minion.saturating_sub(self.size.get().table_minions);
                        self.refresh_events_for_selected_minion();
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
                        self.event_data = IndexMap::new();
                        self.li_events = Vec::new();
                        self.li_minions = Vec::new();
                        self.selected_event = 0;
                        self.selected_cycle = (self.selected_cycle + self.size.get().table_cycles).min(self.cycles_buf.len().saturating_sub(1));
                        if !self.cycles_buf.is_empty() {
                            let sid = self.get_selected_cycle().event().sid().to_string();
                            if let Ok(minions) = self.get_minions(&sid) {
                                self.li_minions = minions;
                                self.selected_minion = 0;
                            }
                        }
                    }
                    ActiveBox::Minions => {
                        self.selected_minion = (self.selected_minion + self.size.get().table_minions).min(self.li_minions.len().saturating_sub(1));
                        self.refresh_events_for_selected_minion();
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
                        self.refresh_events_for_selected_minion();
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
                        } else {
                            self.active_box = ActiveBox::Events;
                            if !self.li_events.is_empty() {
                                self.selected_event = self.li_events.len().saturating_sub(1);
                                self.event_data = self.li_events[self.selected_event].event().flatten();
                            }
                            self.status_at_action_results();
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
                        self.refresh_events_for_selected_minion();
                    }
                    ActiveBox::Events => {
                        if self.selected_event < self.li_events.len().saturating_sub(1) {
                            self.selected_event += 1;
                        } else if !self.info_rows.borrow().is_empty() {
                            self.active_box = ActiveBox::Info;
                            self.actdt_info_offset = 0;
                            self.status_at_action_data();
                            return;
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
                                        self.refresh_events_for_selected_minion();
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
            KeyCode::Char('o') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.evtipc.is_some() {
                    self.open_master_logs();
                } else {
                    self.error_alert_visible = true;
                    self.error_alert_message = "Master is not running".to_string();
                }
            }
            KeyCode::Char('l') if e.modifiers.contains(KeyModifiers::CONTROL) => match self.load_master_logs_local() {
                Ok(()) => {
                    self.master_logs_visible = true;
                    self.master_logs_tab = 0;
                    self.master_logs_polling = false;
                    self.status_at_master_logs();
                }
                Err(e) => {
                    self.error_alert_visible = true;
                    self.error_alert_message = e;
                }
            },
            KeyCode::Char('r') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.registration_form.visible = true;
            }
            KeyCode::Char('a') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(err) = self.load_module_index() {
                    self.error_alert_visible = true;
                    self.error_alert_message = err;
                } else {
                    self.repo_manager.visible = true;
                    self.status_at_repo_manager();
                }
            }
            KeyCode::Char('t') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 1;
            }
            KeyCode::Char('s') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 3;
            }
            KeyCode::Char('e') if e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_confirm_visible = true;
                self.master_confirm_choice = AlertResult::Default;
                self.master_confirm_action = 2;
            }
            KeyCode::Char('m') if !e.modifiers.contains(KeyModifiers::CONTROL) => {
                self.master_menu_visible = true;
                self.master_menu_sel = 0;
                self.status_at_master_menu();
            }
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

    pub fn fetch_cluster_upgrade_status(&self) -> Result<(usize, usize, usize), SysinspectError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                call_master_console(&self.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_UPGRADE_STATUS}"), "*", None, None, None).await.map(|resp| {
                    match resp.payload {
                        ConsolePayload::UpgradeStatus { required, unreachable, pending_post_upgrade } => {
                            (required, unreachable, pending_post_upgrade)
                        }
                        _ => (0, 0, 0),
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

    fn format_machine_message(action: &str, primary_value: Option<&str>, platform: Option<&str>, minion_id: Option<&str>) -> String {
        let mut rows = Vec::new();
        if let Some(value) = primary_value.filter(|v| !v.trim().is_empty()) {
            rows.push((format!("{action}:"), value.trim().to_string()));
        } else {
            rows.push((format!("{action}:"), "OK".to_string()));
        }
        if let Some(value) = platform.filter(|v| !v.trim().is_empty()) {
            rows.push(("Platform:".to_string(), value.trim().to_string()));
        }
        if let Some(value) = minion_id.filter(|v| !v.trim().is_empty()) {
            rows.push(("Machine ID:".to_string(), Self::short_machine_id(value)));
        }
        let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
        rows.into_iter().map(|(label, value)| format!("{label:<width$}  {value}")).collect::<Vec<_>>().join("\n")
    }

    fn format_machine_message_styled(action: &str, primary_value: Option<&str>, platform: Option<&str>, minion_id: Option<&str>) -> Text<'static> {
        let mut rows = Vec::new();
        if let Some(value) = primary_value.filter(|v| !v.trim().is_empty()) {
            rows.push((format!("{action}:"), value.trim().to_string()));
        } else {
            rows.push((format!("{action}:"), "OK".to_string()));
        }
        if let Some(value) = platform.filter(|v| !v.trim().is_empty()) {
            rows.push(("Platform:".to_string(), value.trim().to_string()));
        }
        if let Some(value) = minion_id.filter(|v| !v.trim().is_empty()) {
            rows.push(("Machine ID:".to_string(), Self::short_machine_id(value)));
        }
        let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
        let mut lines: Vec<Line<'static>> = vec![Line::from("")];
        lines.extend(rows.into_iter().map(|(label, value)| {
            Line::from(vec![
                Span::styled(format!("{label:<width$}"), Style::default().fg(palette::SUCCESS_PEAK)),
                Span::styled(format!("  {value}"), Style::default().fg(palette::FG)),
            ])
        }));
        Text::from(lines)
    }

    fn short_machine_id(mid: &str) -> String {
        let trimmed = mid.trim();
        if trimmed.chars().count() <= 8 { trimmed.to_string() } else { format!("{}...", trimmed.chars().take(8).collect::<String>()) }
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
