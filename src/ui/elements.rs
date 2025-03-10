use libeventreg::kvdb::EventSession;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

/// Active box selector
#[derive(Debug, PartialEq, Eq, Default)]
pub enum ActiveBox {
    #[default]
    Cycles,
    Minions,
    Events,
    Info,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum AlertResult {
    #[default]
    Default,
    Purge,
    Quit,
}

pub trait DbListItem {
    fn title(&self) -> String;
    fn event(&self) -> EventSession;
    fn get_list_line(&self, hl: bool) -> Line<'static>;
}

/// Cycle
/// -----
///
/// Cycle is the pointer in the list to the record in the database
#[derive(Debug, Clone)]
pub struct CycleListItem {
    event: EventSession,
    title: String,
}

impl CycleListItem {
    pub fn new(title: &str, event: EventSession) -> Self {
        CycleListItem { event, title: title.to_string() }
    }
}

impl DbListItem for CycleListItem {
    fn title(&self) -> String {
        format!("{} {}", self.event.query(), self.title)
    }

    /// Return event
    fn event(&self) -> EventSession {
        self.event.clone()
    }

    /// Return list line
    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let ttl_fg = if hl { Color::Cyan } else { Color::LightCyan };
        let ts_fg = if hl { Color::Blue } else { Color::LightBlue };
        Line::from(vec![
            Span::styled(self.event().query().to_string(), Style::default().fg(ttl_fg)),
            Span::raw(" "),
            Span::styled(self.event().get_ts_mask(None), Style::default().fg(ts_fg)),
        ])
    }
}

/// Event
/// -----
#[derive(Debug, Clone)]
pub struct EventListItem {
    title: String,
}

impl EventListItem {
    pub fn new(title: &str) -> Self {
        EventListItem { title: title.to_string() }
    }
}

impl DbListItem for EventListItem {
    fn title(&self) -> String {
        self.title.clone()
    }

    /// Stub
    fn event(&self) -> EventSession {
        EventSession::new("".to_string(), "".to_string(), chrono::Utc::now())
    }

    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let fg = if hl { Color::White } else { Color::Gray };
        Line::from(vec![Span::styled(self.title(), Style::default().fg(fg))])
    }
}

/// Minion
/// ------
#[derive(Debug, Clone)]
pub struct MinionListItem {
    title: String,
}

impl MinionListItem {
    pub fn new(title: &str) -> Self {
        MinionListItem { title: title.to_string() }
    }
}

impl DbListItem for MinionListItem {
    fn title(&self) -> String {
        self.title.clone()
    }

    /// Stub
    fn event(&self) -> EventSession {
        EventSession::new("".to_string(), "".to_string(), chrono::Utc::now())
    }

    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let fg = if hl { Color::White } else { Color::Gray };
        Line::from(vec![Span::styled(self.title(), Style::default().fg(fg))])
    }
}
