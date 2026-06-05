use crate::{MEM_LOGGER, call_master_console, ui::elements::DbListItem};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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
    commands::{CLUSTER_MINION_INFO, CLUSTER_MODELS, CLUSTER_ONLINE_MINIONS, CLUSTER_TRAITS_UPDATE},
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::Line,
    widgets::{Paragraph, Row},
};
use ratatui_cheese::tree::TreeState;
use std::{
    cell::{Cell, RefCell},
    io::{self, Error},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

mod alert;
mod dslbrowser;
mod elements;
mod online;
mod palette;
mod statusbar;
mod traittag;
mod typecolors;
mod wgt;

pub async fn run(cfg: MasterConfig) -> io::Result<()> {
    match SysInspectUX::new(cfg.clone()).await {
        Ok(mut app) => {
            let mut terminal = ratatui::init();
            let r = app.run(&mut terminal);
            ratatui::restore();

            // XXX: Temporary log dumper. Should go to its own window popup later
            if !MEM_LOGGER.get_messages().is_empty() {
                println!("Memory log:");
                println!("{:#?}", MEM_LOGGER.get_messages());
            }

            r
        }
        Err(err) => Err(Error::new(io::ErrorKind::InvalidData, err)),
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
pub struct SysInspectUX {
    exit: bool,
    pub selected_cycle: usize,
    pub selected_minion: usize,
    pub selected_event: usize,

    pub li_minions: Vec<MinionListItem>,
    pub li_events: Vec<EventListItem>,
    pub event_data: IndexMap<String, String>,
    pub active_box: ActiveBox,

    pub status_text: Line<'static>,

    /// Purge alert
    pub purge_alert_visible: bool,
    pub purge_alert_choice: AlertResult,

    /// Error alert
    pub error_alert_visible: bool,
    pub error_alert_message: String,
    pub error_alert_choice: AlertResult,

    /// Exit alert
    pub exit_alert_visible: bool,
    pub exit_alert_choice: AlertResult,

    // Help popup
    pub help_popup_visible: bool,

    // Online minions popup
    pub online_minions_visible: bool,
    pub online_minions_rows: Vec<ConsoleOnlineMinionRow>,
    pub online_minions_selected: usize,
    pub online_minions_show_alive: bool,
    pub online_minions_focus: usize,
    pub online_minions_info_visible: bool,
    pub online_minions_info_rows: Vec<ConsoleMinionInfoRow>,
    pub online_minions_tree_state: Option<TreeState>,
    pub online_minions_info_modified: bool,

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
            status_text: Line::from(vec![]),

            // Alerts
            purge_alert_visible: false,
            purge_alert_choice: AlertResult::default(),
            exit_alert_visible: false,
            exit_alert_choice: AlertResult::default(),
            error_alert_visible: false,
            error_alert_choice: AlertResult::default(),
            error_alert_message: String::new(),
            help_popup_visible: false,

            online_minions_visible: false,
            online_minions_rows: Vec::new(),
            online_minions_selected: 0,
            online_minions_show_alive: true,
            online_minions_focus: 2,
            online_minions_info_visible: false,
            online_minions_info_rows: Vec::new(),
            online_minions_tree_state: None,
            online_minions_info_modified: false,

            tag_visible: false,
            tag_key_buf: String::new(),
            tag_val_buf: String::new(),
            tag_focus: 0,
            tag_pos: 0,

            evtipc: None,
            dsl_browser: dslbrowser::DslBrowser::new(),
            cfg: MasterConfig::default(),
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

    pub fn run(&mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        self.cycles_buf = self.get_cycles().unwrap();

        while !self.exit {
            term.draw(|frame| self.draw(frame))?;
            self.on_events()?;
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

        let status_paragraph = Paragraph::new(self.status_text.clone()).style(Style::default().fg(self::palette::GRAY_1).bg(self::palette::BG_1));
        frame.render_widget(status_paragraph, status_area);
    }

    fn on_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(e) = event::read()?
                && e.kind == KeyEventKind::Press
            {
                self.on_key(e);
            }
        } else {
            if let Ok(cycles) = self.get_cycles() {
                self.cycles_buf = cycles;
            }
            if self.online_minions_visible {
                self.refresh_online_minions();
            }
        }
        Ok(())
    }

    /// Cycle active pan to the right (used on RIGHT or ENTER key)
    fn shift_next(&mut self) {
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

    /// Process online minions popup key events
    fn on_online_minions_popup(&mut self, e: event::KeyEvent) -> bool {
        if !self.online_minions_visible {
            return false;
        }
        if e.code == KeyCode::Char('t') {
            if !self.online_minions_show_alive {
                self.error_alert_visible = true;
                self.error_alert_message = "Offline minions cannot be tagged live. Do it manually.".to_string();
            } else {
                let filtered = self.filtered_minions();
                if filtered.is_empty() || filtered.len() <= self.online_minions_selected {
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
            return true;
        }
        match e.code {
            KeyCode::Tab => {
                self.online_minions_focus = (self.online_minions_focus + 1) % 4;
            }
            KeyCode::BackTab => {
                self.online_minions_focus = (self.online_minions_focus + 3) % 4;
            }
            KeyCode::Enter | KeyCode::Char('i') if self.online_minions_focus == 2 => {
                self.online_minions_info_visible = !self.online_minions_info_visible;
                self.online_minions_info_rows = Vec::new();
                self.online_minions_tree_state = None;
                if self.online_minions_info_visible {
                    self.online_minions_info_modified = false;
                    self.load_selected_minion_info();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.online_minions_focus {
                0 => {
                    self.online_minions_visible = false;
                    self.restore_status();
                }
                1 => {
                    self.online_minions_show_alive = !self.online_minions_show_alive;
                    self.online_minions_selected = 0;
                    self.online_minions_info_modified = false;
                    self.load_selected_minion_info();
                }
                3 => {
                    if let Some(ref mut ts) = self.online_minions_tree_state {
                        ts.toggle_selected();
                    }
                }
                _ => {}
            },
            KeyCode::Esc => {
                self.online_minions_visible = false;
                self.restore_status();
            }
            KeyCode::Up => match self.online_minions_focus {
                2 => {
                    self.online_minions_selected = self.online_minions_selected.saturating_sub(1);
                    self.online_minions_info_modified = false;
                    self.load_selected_minion_info();
                }
                3 => {
                    if let Some(ref mut ts) = self.online_minions_tree_state {
                        let groups = SysInspectUX::build_info_tree(&self.online_minions_info_rows);
                        ts.select_prev(&groups);
                    }
                }
                _ => {}
            },
            KeyCode::Down => match self.online_minions_focus {
                2 => {
                    let filtered = self.filtered_minions();
                    self.online_minions_selected = (self.online_minions_selected + 1).min(filtered.len().saturating_sub(1));
                    self.online_minions_info_modified = false;
                    self.load_selected_minion_info();
                }
                3 => {
                    if let Some(ref mut ts) = self.online_minions_tree_state {
                        let groups = SysInspectUX::build_info_tree(&self.online_minions_info_rows);
                        ts.select_next(&groups);
                    }
                }
                _ => {}
            },
            KeyCode::Right => {
                if self.online_minions_focus == 3
                    && let Some(ref mut ts) = self.online_minions_tree_state
                {
                    let (group, _) = ts.selected();
                    ts.expand(group);
                }
            }
            KeyCode::Left => {
                if self.online_minions_focus == 3
                    && let Some(ref mut ts) = self.online_minions_tree_state
                {
                    let (group, _) = ts.selected();
                    ts.collapse(group);
                }
            }
            KeyCode::PageUp if self.online_minions_focus == 2 => {
                self.online_minions_selected = self.online_minions_selected.saturating_sub(10);
                self.online_minions_info_modified = false;
                self.load_selected_minion_info();
            }
            KeyCode::PageDown if self.online_minions_focus == 2 => {
                let filtered = self.filtered_minions();
                self.online_minions_selected = (self.online_minions_selected + 10).min(filtered.len().saturating_sub(1));
                self.online_minions_info_modified = false;
                self.load_selected_minion_info();
            }
            _ => {}
        }
        true
    }

    fn filtered_minions(&self) -> Vec<&ConsoleOnlineMinionRow> {
        self.online_minions_rows.iter().filter(|r| r.alive == self.online_minions_show_alive).collect()
    }

    fn restore_status(&mut self) {
        match self.active_box {
            ActiveBox::Cycles => self.status_at_cycles(),
            ActiveBox::Minions => self.status_at_minions(),
            ActiveBox::Events => self.status_at_action_results(),
            ActiveBox::Info => self.status_at_action_data(),
        }
    }

    fn refresh_online_minions(&mut self) {
        if let Ok(rows) = self.get_online_minions() {
            let old_mid = self.filtered_minions().get(self.online_minions_selected).map(|r| r.minion_id.clone());
            self.online_minions_rows = rows;
            if let Some(mid) = old_mid {
                let filtered = self.filtered_minions();
                if let Some(pos) = filtered.iter().position(|r| r.minion_id == mid) {
                    self.online_minions_selected = pos;
                } else {
                    self.online_minions_selected = 0;
                    self.online_minions_tree_state = None;
                    self.online_minions_info_modified = false;
                    self.load_selected_minion_info();
                }
            }
        }
    }

    fn load_selected_minion_info(&mut self) {
        if self.online_minions_info_modified {
            return;
        }
        let filtered: Vec<&ConsoleOnlineMinionRow> = self.online_minions_rows.iter().filter(|r| r.alive == self.online_minions_show_alive).collect();
        if let Some(row) = filtered.get(self.online_minions_selected) {
            let expanded: Vec<String> = if let Some(ref ts) = self.online_minions_tree_state {
                let old_groups = SysInspectUX::build_info_tree(&self.online_minions_info_rows);
                old_groups.iter().enumerate().filter(|(i, _)| ts.is_expanded(*i)).map(|(_, g)| g.header().text().to_string()).collect()
            } else {
                Vec::new()
            };
            match self.get_minion_info(&row.minion_id) {
                Ok(rows) => {
                    let new_groups = SysInspectUX::build_info_tree(&rows);
                    let mut ts = TreeState::new(new_groups.len());
                    for (i, g) in new_groups.iter().enumerate() {
                        if expanded.contains(&g.header().text().to_string()) {
                            ts.expand(i);
                        }
                    }
                    self.online_minions_tree_state = Some(ts);
                    self.online_minions_info_rows = rows;
                }
                Err(_) => {
                    self.online_minions_info_rows = Vec::new();
                    self.online_minions_tree_state = None;
                }
            }
        }
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
                    && let Some(val) = self.online_minions_info_rows.iter().find(|r| r.key == self.tag_key_buf)
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
        let filtered = self.filtered_minions();
        let mid = match filtered.get(self.online_minions_selected) {
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
        let expanded: Vec<String> = if let Some(ref ts) = self.online_minions_tree_state {
            let old_groups = SysInspectUX::build_info_tree(&self.online_minions_info_rows);
            old_groups.iter().enumerate().filter(|(i, _)| ts.is_expanded(*i)).map(|(_, g)| g.header().text().to_string()).collect()
        } else {
            Vec::new()
        };
        if val.is_empty() {
            self.online_minions_info_rows.retain(|r| r.key != key);
        } else if let Some(existing) = self.online_minions_info_rows.iter_mut().find(|r| r.key == key) {
            existing.value = serde_json::Value::String(val);
        } else {
            self.online_minions_info_rows.push(ConsoleMinionInfoRow {
                key,
                value: serde_json::Value::String(val),
                source: libsysinspect::traits::TraitSource::Static,
            });
        }
        let new_groups = SysInspectUX::build_info_tree(&self.online_minions_info_rows);
        let mut ts = TreeState::new(new_groups.len());
        for (i, g) in new_groups.iter().enumerate() {
            if expanded.contains(&g.header().text().to_string()) {
                ts.expand(i);
            }
        }
        self.online_minions_tree_state = Some(ts);
        self.online_minions_info_modified = true;
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
                    let missing = if model == "(select)" || model == "(no models found)" { "Model" } else { "Target" };
                    self.error_alert_visible = true;
                    self.error_alert_message = format!("Select {missing} first!");
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

        if self.on_exit_alert(e) {
            return;
        }

        if self.on_error_alert(e) {
            return;
        }

        if self.on_tag_popup(e) {
            return;
        }

        if self.on_online_minions_popup(e) {
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
                self.exit_alert_choice = AlertResult::Default;
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
                    if let Ok(minions) = self.get_online_minions() {
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
            KeyCode::Char('o') => match self.get_online_minions() {
                Ok(rows) if rows.is_empty() => {
                    self.error_alert_visible = true;
                    self.error_alert_message = "No minions registered yet".to_string();
                }
                Ok(rows) => {
                    let has_online = rows.iter().any(|r| r.alive);
                    self.online_minions_rows = rows;
                    self.online_minions_show_alive = has_online;
                    self.online_minions_visible = true;
                    self.online_minions_focus = 2;
                    self.online_minions_selected = 0;
                    self.online_minions_tree_state = None;
                    self.load_selected_minion_info();
                    self.status_at_online_minions();
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
    pub fn get_online_minions(&self) -> Result<Vec<ConsoleOnlineMinionRow>, SysinspectError> {
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
        s.lines().map(|l| l.len() as u16).max().unwrap_or_default()
    }
}
