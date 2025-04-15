use crate::MEM_LOGGER;
use crate::ui::elements::DbListItem;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use elements::{ActiveBox, AlertResult, CycleListItem, EventListItem, MinionListItem};
use indexmap::IndexMap;
use libeventreg::{
    QUERY_CMD_PURGE_ALL, QUERY_CYCLES, QUERY_EVENTS, QUERY_MINIONS,
    ipcc::DbIPCClient,
    kvdb::{EventData, EventMinion, EventSession},
};
use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Paragraph, Row},
};
use std::{
    cell::{Cell, RefCell},
    io::{self, Error},
    sync::Arc,
};
use tokio::sync::Mutex;

mod alert;
mod elements;
mod statusbar;
mod wgt;

pub async fn run(cfg: MasterConfig) -> io::Result<()> {
    match SysInspectUX::new(cfg.telemetry_socket().to_str().unwrap_or_default()).await {
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

    // DB
    pub evtipc: Option<Arc<Mutex<DbIPCClient>>>,

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

            evtipc: None,
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
    pub async fn new(ipc_socket: &str) -> Result<Self, SysinspectError> {
        let mut ux = SysInspectUX::default();

        ux.evtipc = Some(Arc::new(Mutex::new(DbIPCClient::new(ipc_socket).await?)));

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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
            .split(frame.area());
        let main_area = chunks[0];
        let status_area = chunks[1];

        frame.render_widget(self, main_area);

        let status_paragraph = Paragraph::new(self.status_text.clone())
            .style(Style::default().fg(Color::Yellow).bg(Color::Blue).add_modifier(Modifier::BOLD));
        frame.render_widget(status_paragraph, status_area);
    }

    fn on_events(&mut self) -> io::Result<()> {
        if let Event::Key(e) = event::read()? {
            if e.kind == KeyEventKind::Press {
                self.on_key(e);
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
                    self.exit();
                }
                _ => {}
            }
        }

        stat
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

    /// Update minions on up/down keystrokes
    fn on_update_minions(&mut self, down: bool) {}

    /// Update events on up/down keystrokes
    fn on_update_events(&mut self, down: bool) {}

    fn on_key(&mut self, e: event::KeyEvent) {
        if self.on_purge_alert(e) {
            return;
        }

        if self.on_exit_alert(e) {
            return;
        }

        if self.on_error_alert(e) {
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
                        self.selected_cycle =
                            (self.selected_cycle + self.size.get().table_cycles).min(self.cycles_buf.len().saturating_sub(1));
                    }
                    ActiveBox::Minions => {
                        if !self.li_events.is_empty() {
                            self.selected_minion = (self.selected_minion + self.size.get().table_minions)
                                .min(self.li_minions.len().saturating_sub(1));
                        }
                    }
                    ActiveBox::Events => {
                        if !self.li_events.is_empty() {
                            self.selected_event =
                                (self.selected_event + self.size.get().table_events).min(self.li_events.len().saturating_sub(1));
                        }
                    }
                    ActiveBox::Info => {
                        self.actdt_info_offset = (self.actdt_info_offset + self.size.get().table_info)
                            .min(self.info_rows.borrow().len().saturating_sub(1));
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
                    ActiveBox::Events => {
                        if !self.li_events.is_empty() && self.get_selected_event().is_some() {
                            self.status_at_action_data();
                            self.active_box = ActiveBox::Info;
                            self.event_data = self.get_selected_event().unwrap().event().flatten();
                        }
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

            KeyCode::BackTab => {
                if self.active_box == ActiveBox::Info {
                    self.status_at_action_results();
                    self.active_box = ActiveBox::Events;
                }
            }

            KeyCode::Tab => {
                if self.active_box == ActiveBox::Events {
                    self.status_at_action_data();
                    self.active_box = ActiveBox::Info;
                }
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
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {}", err)));
                        }
                    };

                    let mut cycles: Vec<CycleListItem> = rq
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            let s =
                                EventSession::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec())
                                    .unwrap();
                            CycleListItem::new(s.get_ts_mask(None).as_str(), s)
                        })
                        .collect();
                    cycles.sort_by_key(|ts| ts.event().get_ts_unix());
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
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {}", err)));
                        }
                    };
                    let minions: Vec<MinionListItem> = r
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            let mut s =
                                EventMinion::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec())
                                    .unwrap();
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
                            return Err(SysinspectError::ProtoError(format!("Error getting data: {}", err)));
                        }
                    };
                    let events: Vec<EventListItem> = r
                        .into_inner()
                        .records
                        .into_iter()
                        .map(|rec| {
                            EventListItem::new(
                                EventData::from_bytes(String::from_utf8(rec.value).unwrap_or_default().as_bytes().to_vec())
                                    .unwrap(),
                            )
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
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    match c_ipc.lock().await.query("", "", "", QUERY_CMD_PURGE_ALL).await {
                        Ok(_) => Ok(()),
                        Err(err) => Err(SysinspectError::ProtoError(format!("Error purging data: {}", err))),
                    }
                })
            });
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

    /// Count the vertical space for the alert display, plus three empty lines
    fn get_text_lines(s: &str) -> u16 {
        s.matches('\n').count() as u16 + 3
    }

    /// Get the maximum width of the lines
    fn get_max_width_lines(s: &str) -> u16 {
        s.lines().map(|l| l.len() as u16).max().unwrap_or_default()
    }
}
