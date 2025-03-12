use crate::MEM_LOGGER;
use crate::ui::elements::DbListItem;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use elements::{ActiveBox, AlertResult, CycleListItem, EventListItem, MinionListItem};
use indexmap::IndexMap;
use libeventreg::{
    QUERY_CYCLES, QUERY_EVENTS, QUERY_MINIONS,
    ipcc::DbIPCClient,
    kvdb::{EventData, EventMinion, EventSession},
};
use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};
use std::{
    io::{self, Error},
    sync::Arc,
};
use tokio::sync::Mutex;

mod alert;
mod elements;
mod wgt;

pub async fn run(cfg: MasterConfig) -> io::Result<()> {
    match SysInspectUX::new(cfg.telemetry_socket().to_str().unwrap_or_default()).await {
        Ok(mut app) => {
            let mut terminal = ratatui::init();
            let r = app.run(&mut terminal);
            ratatui::restore();

            println!("{:#?}", MEM_LOGGER.get_messages());

            r
        }
        Err(err) => Err(Error::new(io::ErrorKind::InvalidData, err)),
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

    pub status_text: String,

    /// Purge alert
    pub purge_alert_visible: bool,
    pub purge_alert_choice: AlertResult,

    /// Exit alert
    pub exit_alert_visible: bool,
    pub exit_alert_choice: AlertResult,

    // DB
    pub evtipc: Option<Arc<Mutex<DbIPCClient>>>,

    // Buffers
    pub cycles_buf: Vec<CycleListItem>,
    pub minions_buf: Vec<MinionListItem>,
    pub events_buf: Vec<EventListItem>,
}

impl Default for SysInspectUX {
    fn default() -> Self {
        Self {
            exit: false,
            selected_cycle: 0,
            selected_minion: 0,
            selected_event: 0,
            li_minions: Vec::new(),
            li_events: Vec::new(),
            event_data: IndexMap::new(),
            active_box: ActiveBox::default(),
            status_text: String::new(),
            purge_alert_visible: false,
            purge_alert_choice: AlertResult::default(),
            exit_alert_visible: false,
            exit_alert_choice: AlertResult::default(),
            evtipc: None,
            cycles_buf: Vec::new(),
            minions_buf: Vec::new(),
            events_buf: Vec::new(),
        }
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

        let status_paragraph = Paragraph::new(self.status_text.as_str())
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
            ActiveBox::Cycles => self.active_box = ActiveBox::Minions,
            ActiveBox::Minions => self.active_box = ActiveBox::Events,
            ActiveBox::Events | ActiveBox::Info => self.active_box = ActiveBox::Cycles,
        };
    }

    /// Cycle active pan to the left (used on LEFT or ESC key)
    fn shift_prev(&mut self) {
        match self.active_box {
            ActiveBox::Cycles => self.active_box = ActiveBox::Events,
            ActiveBox::Minions => self.active_box = ActiveBox::Cycles,
            ActiveBox::Events | ActiveBox::Info => self.active_box = ActiveBox::Minions,
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
                        self.status_text = "Purge confirmed".to_string();
                        // XXX: call DB here
                    } else {
                        self.status_text = "Purge cancelled".to_string();
                    }
                    self.purge_alert_visible = false;
                }
                KeyCode::Esc => {
                    self.status_text = "Purge cancelled".to_string();
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

    fn on_key(&mut self, e: event::KeyEvent) {
        if self.on_purge_alert(e) {
            return;
        }

        if self.on_exit_alert(e) {
            return;
        }

        match e.code {
            KeyCode::Up => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        if self.selected_cycle > 0 {
                            self.selected_cycle -= 1;
                        }
                    }
                    ActiveBox::Minions => {
                        if self.selected_minion > 0 {
                            self.selected_minion -= 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event > 0 {
                            self.selected_event -= 1;
                        }
                    }
                    ActiveBox::Info => self.active_box = ActiveBox::Events,
                };
            }
            KeyCode::Down => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        let cycles = self.get_cycles();
                        if self.selected_cycle < cycles.len().saturating_sub(1) {
                            self.selected_cycle += 1;
                        }
                    }
                    ActiveBox::Minions => {
                        if self.selected_minion < self.li_minions.len().saturating_sub(1) {
                            self.selected_minion += 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event < self.li_events.len().saturating_sub(1) {
                            self.selected_event += 1;
                        }
                    }
                    _ => {}
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
                        self.cycles_buf = self.get_cycles();
                        if !self.cycles_buf.is_empty() {
                            self.li_minions = self.get_minions(self.get_selected_cycle().event().sid());
                            self.selected_minion = 0;
                        }
                        self.shift_next();
                    }
                    ActiveBox::Minions => {
                        if !self.li_minions.is_empty() {
                            if let Some(mli) = self.get_selected_minion() {
                                self.li_events = self.get_events(self.get_selected_cycle().event().sid(), mli.event().id());
                            }
                            self.selected_event = 0;
                        }
                        self.shift_next();
                    }
                    ActiveBox::Events => {
                        if !self.li_events.is_empty() {
                            self.active_box = ActiveBox::Info;
                            self.event_data = self.get_event_data();
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

    fn exit(&mut self) {
        self.exit = true;
    }
    /// Returns a vector of cycle names.
    pub fn get_cycles(&self) -> Vec<CycleListItem> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let r = c_ipc.lock().await.query("", "", "", QUERY_CYCLES).await.unwrap();
                    let cycles: Vec<CycleListItem> = r
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
                    cycles
                })
            });
        }
        vec![]
    }

    /// Returns a vector of minion names (random IDs).
    /// Params:
    /// - `sid`: Session ID (cycle)
    pub fn get_minions(&self, sid: &str) -> Vec<MinionListItem> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let r = c_ipc.lock().await.query("", "", sid, QUERY_MINIONS).await.unwrap();
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
                    minions
                })
            });
        }

        vec![]
    }

    /// Returns a vector of events for a particular minion.
    /// Params:
    /// - `sid`: Session ID (cycle)
    /// - `mid`: Minion ID
    pub fn get_events(&self, sid: &str, mid: &str) -> Vec<EventListItem> {
        if let Some(ipc) = self.evtipc.as_ref() {
            let c_ipc = ipc.clone();
            return tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let r = c_ipc.lock().await.query(mid, "", sid, QUERY_EVENTS).await.unwrap();
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
                    events
                })
            });
        }
        vec![]
    }

    /// Count the vertical space for the alert display, plus three empty lines
    fn get_text_lines(s: &str) -> u16 {
        s.matches('\n').count() as u16 + 3
    }

    /// Get event data
    fn get_event_data(&self) -> IndexMap<String, String> {
        let mut m = IndexMap::new();
        m.insert("Foo".to_string(), "Bar".to_string());
        m.insert("Baz".to_string(), "Toto".to_string());
        m
    }
}
