use libeventreg::kvdb::{EventData, EventMinion, EventSession};
use libsysinspect::{
    traits::{SYS_NET_HOSTNAME, SYS_NET_HOSTNAME_FQDN, SYS_NET_HOSTNAME_IP},
    util::dataconv::{as_int, as_str},
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Row},
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
            Span::styled(self.event().get_ts_mask(None), Style::default().fg(ts_fg)),
            Span::raw(" "),
            Span::styled(self.event().query().to_string(), Style::default().fg(ttl_fg)),
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

    // Right-alight a string within the width
    fn right_align(s: &str, width: usize) -> String {
        let len = s.chars().count();
        if len >= width { s.to_string() } else { format!("{s:>width$}") }
    }

    // Yellow cell
    pub fn yc(v: String, keywidth: usize) -> Cell<'static> {
        Cell::from(Self::right_align(&v, keywidth)).style(Style::default().fg(Color::LightYellow))
    }

    // Grey cell
    pub fn gc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(Color::Gray))
    }

    // Green cell
    pub fn grc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(Color::LightGreen))
    }

    // Red cell
    pub fn rc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD))
    }

    /// Get events data table
    pub fn get_event_table(&self, keywidth: usize) -> Vec<Row<'_>> {
        vec![
            Row::new(vec![Self::yc("Info:".to_string(), keywidth), Self::gc(as_str(self.event.get_response().get("message").cloned()))]),
            Row::new(vec![
                Self::yc("Return code:".to_string(), keywidth),
                if as_int(self.event.get_response().get("retcode").cloned()) == 0 {
                    Self::grc("Success".to_string())
                } else {
                    Self::rc(format!("Error - {}", as_int(self.event.get_response().get("retcode").cloned())))
                },
            ]),
            Row::new(vec![Self::yc("Occurred:".to_string(), keywidth), Self::gc(self.event.get_timestamp())]),
            Row::new(vec![
                Self::yc("Scope:".to_string(), keywidth),
                Self::gc(if self.event.get_status_id() == "$" { "Global".to_string() } else { self.event.get_status_id() }),
            ]),
        ]
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

pub struct HostInfo {
    pub hostname: String,
    pub ipaddr: String,
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

    /// Return (IP address, hostname)
    fn hostname(&self) -> HostInfo {
        let ipaddr = as_str(self.event.get_trait(SYS_NET_HOSTNAME_IP).cloned());
        let mut hostname = as_str(self.event.get_trait(SYS_NET_HOSTNAME_FQDN).cloned());
        if hostname.is_empty() {
            hostname = as_str(self.event.get_trait(SYS_NET_HOSTNAME).cloned());
        }
        HostInfo { ipaddr: if !ipaddr.is_empty() { ipaddr } else { "127.0.0.1".to_string() }, hostname }
    }
}

impl DbListItem for MinionListItem {
    type EventType = EventMinion;

    /// Return event object
    fn event(&self) -> EventMinion {
        self.event.clone()
    }

    /// Return list line
    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let ttl_fg = if hl { Color::Cyan } else { Color::LightCyan };
        let ts_fg = if hl { Color::Blue } else { Color::LightBlue };
        let HostInfo { ipaddr, hostname } = self.hostname();
        Line::from(vec![Span::styled(ipaddr, Style::default().fg(ts_fg)), Span::raw(" "), Span::styled(hostname, Style::default().fg(ttl_fg))])
    }

    fn title(&self) -> String {
        let HostInfo { ipaddr, hostname } = self.hostname();
        format!("{ipaddr} ({hostname})")
    }
}
