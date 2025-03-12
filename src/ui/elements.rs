use libeventreg::kvdb::{EventData, EventMinion, EventSession};
use libsysinspect::{
    traits::{SYS_NET_HOSTNAME, SYS_NET_HOSTNAME_FQDN, SYS_NET_HOSTNAME_IP},
    util::dataconv::as_str,
};
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
    type EventType;

    fn title(&self) -> String;
    fn event(&self) -> Self::EventType;
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
    type EventType = EventSession;

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
    event: EventData,
}

impl EventListItem {
    pub fn new(event: EventData) -> Self {
        EventListItem { event }
    }
}

impl DbListItem for EventListItem {
    type EventType = EventData;

    fn title(&self) -> String {
        as_str(self.event.get_constraints().get("descr").cloned())
    }

    /// Stub
    fn event(&self) -> EventData {
        self.event.clone()
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
    event: EventMinion,
}

impl MinionListItem {
    pub fn new(event: EventMinion) -> Self {
        MinionListItem { event }
    }
}

impl DbListItem for MinionListItem {
    type EventType = EventMinion;

    /// Return title
    fn title(&self) -> String {
        let ipaddr = as_str(self.event.get_trait(SYS_NET_HOSTNAME_IP).cloned());
        let mut hostname = as_str(self.event.get_trait(SYS_NET_HOSTNAME_FQDN).cloned());
        if hostname.is_empty() {
            hostname = as_str(self.event.get_trait(SYS_NET_HOSTNAME).cloned());
        }
        format!("{} - {}", hostname, if !ipaddr.is_empty() { ipaddr } else { "127.0.0.1".to_string() })
    }

    /// Return event object
    fn event(&self) -> EventMinion {
        self.event.clone()
    }

    /// Return list line
    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let fg = if hl { Color::White } else { Color::Gray };
        Line::from(vec![Span::styled(self.title(), Style::default().fg(fg))])
    }
}
