use libeventreg::kvdb::{EventData, EventMinion, EventSession};
use libsysinspect::{
    traits::{SYS_NET_HOSTNAME, SYS_NET_HOSTNAME_FQDN, SYS_NET_HOSTNAME_IP},
    util::dataconv::{as_int, as_str},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Row},
};

use super::palette;

/// Active box selector
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
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
    Close,
    ClusterConfirm,
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

    fn display_query(query: &str) -> String {
        query.strip_suffix("/$").unwrap_or(query).to_string()
    }

    fn query_spans(query: &str, dimmed: bool) -> Vec<Span<'static>> {
        let display = Self::display_query(query);
        if dimmed {
            return vec![Span::styled(display, Style::default().fg(palette::FG))];
        }

        if let Some((model, label)) = display.split_once(':') {
            return vec![
                Span::styled(model.to_string(), Style::default().fg(palette::PROCESSING_BASE)),
                Span::styled(":", Style::default().fg(palette::FG)),
                Span::styled(label.to_string(), Style::default().fg(palette::PROCESSING_PEAK)),
            ];
        }

        let parts: Vec<&str> = display.split('/').collect();
        match parts.as_slice() {
            [model, target] => vec![
                Span::styled((*model).to_string(), Style::default().fg(palette::PROCESSING_BASE)),
                Span::styled("/", Style::default().fg(palette::FG)),
                Span::styled((*target).to_string(), Style::default().fg(palette::PROCESSING_HEAT)),
            ],
            [model, target, state] => vec![
                Span::styled((*model).to_string(), Style::default().fg(palette::PROCESSING_BASE)),
                Span::styled("/", Style::default().fg(palette::FG)),
                Span::styled((*target).to_string(), Style::default().fg(palette::PROCESSING_HEAT)),
                Span::styled("/", Style::default().fg(palette::FG)),
                Span::styled((*state).to_string(), Style::default().fg(palette::PRIMARY)),
            ],
            _ => vec![Span::styled(display, Style::default().fg(palette::FG))],
        }
    }
}

impl DbListItem for CycleListItem {
    type EventType = EventSession;

    fn title(&self) -> String {
        format!("{} {}", Self::display_query(self.event.query()), self.title)
    }

    /// Return event
    fn event(&self) -> EventSession {
        self.event.clone()
    }

    /// Return list line
    fn get_list_line(&self, hl: bool) -> Line<'static> {
        let mut spans = vec![Span::styled(self.event().get_ts_mask(Some("%m-%d %H:%M")), Style::default().fg(palette::GRAY_1)), Span::raw(" ")];
        spans.extend(Self::query_spans(self.event().query(), hl));
        Line::from(spans)
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

    // Key cell
    pub fn yc(v: String, keywidth: usize) -> Cell<'static> {
        Cell::from(Self::right_align(&v, keywidth)).style(Style::default().fg(palette::GRAY_1))
    }

    // Value cell
    pub fn gc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(palette::FG))
    }

    // Success cell
    pub fn grc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(palette::SUCCESS))
    }

    // Error cell
    pub fn rc(v: String) -> Cell<'static> {
        Cell::from(v).style(Style::default().fg(palette::ERROR).add_modifier(Modifier::BOLD))
    }

    pub fn get_aligned_line(&self, left_pad: usize) -> Line<'static> {
        let arrow = " \u{27A4}  ";
        let t = self.title().replace(" with ", arrow);
        if let Some(pos) = t.find(arrow) {
            let left = right_pad(&t[..pos], left_pad);
            let after = &t[pos + arrow.len()..];
            Line::from(vec![
                Span::styled(left, Style::default().fg(palette::FG)),
                Span::styled(arrow, Style::default().fg(palette::PROCESSING).add_modifier(Modifier::BOLD)),
                Span::styled(after.to_string(), Style::default().fg(palette::FG)),
            ])
        } else {
            Line::from(vec![Span::styled(t, Style::default().fg(palette::FG))])
        }
    }

    pub fn left_width(&self) -> usize {
        let arrow = " \u{27A4}  ";
        self.title().replace(" with ", arrow).find(arrow).unwrap_or(0)
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

    fn get_list_line(&self, _hl: bool) -> Line<'static> {
        self.get_aligned_line(0)
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
        let _ = hl;
        let HostInfo { ipaddr, hostname } = self.hostname();
        Line::from(vec![
            Span::styled(format_ip_octets(&ipaddr), Style::default().fg(palette::GRAY_1)),
            Span::raw(" "),
            Span::styled(hostname, Style::default().fg(palette::PROCESSING_PEAK)),
        ])
    }

    fn title(&self) -> String {
        let HostInfo { ipaddr, hostname } = self.hostname();
        format!("{ipaddr} ({hostname})")
    }
}

fn format_ip_octets(ip: &str) -> String {
    let octets: Vec<&str> = ip.split('.').collect();
    if octets.len() == 4 { format!("{:>3}.{:>3}.{:>3}.{:>3}", octets[0], octets[1], octets[2], octets[3]) } else { format!("{:>15}", ip) }
}

fn right_pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width { s.to_string() } else { format!("{}{}", s, " ".repeat(width - len)) }
}
