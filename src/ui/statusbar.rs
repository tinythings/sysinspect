use super::{SysInspectUX, palette};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

impl SysInspectUX {
    pub(crate) fn status_at_cycles(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" ENTER ", Style::default().fg(palette::FG)),
            Span::styled("to select,  ", Style::default().fg(palette::FAINT)),
            Span::styled("RIGHT ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("LEFT ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ESC ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_minions(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" ENTER ", Style::default().fg(palette::FG)),
            Span::styled("to select,  ", Style::default().fg(palette::FAINT)),
            Span::styled("RIGHT ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("LEFT ", Style::default().fg(palette::FG)),
            Span::styled("to query calls,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ESC ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_action_results(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" TAB ", Style::default().fg(palette::FG)),
            Span::styled("to action data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ENTER ", Style::default().fg(palette::FG)),
            Span::styled("to select an action data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("RIGHT ", Style::default().fg(palette::FG)),
            Span::styled("to query calls,  ", Style::default().fg(palette::FAINT)),
            Span::styled("LEFT ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ESC ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_action_data(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" SHIFT+TAB ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Up/Down ", Style::default().fg(palette::FG)),
            Span::styled("to scroll the data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("LEFT ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ESC ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_online_minions(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![
            key("\u{2190}\u{2192} "),
            desc("switch pane,  "),
            key("\u{2191}\u{2193} "),
            desc("navigate,  "),
            key("Enter "),
            desc("minion info,  "),
            key("Tab "),
            desc("cycle focus,  "),
            key("PgUp/PgDn "),
            desc("skip rows,  "),
            key("'t' "),
            desc("tag trait,  "),
            key("ESC "),
            desc("close"),
        ]);
    }

    pub(crate) fn status_at_minion_info(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![
            key("\u{2191}\u{2193} "),
            desc("navigate,  "),
            key("Enter "),
            desc("collapse/expand,  "),
            key("\u{2190}\u{2192} "),
            desc("fold,  "),
            key("PgUp/PgDn "),
            desc("skip,  "),
            key("Tab "),
            desc("filter,  "),
            key("+/- "),
            desc("expand/collapse all,  "),
            key("ESC "),
            desc("back"),
        ]);
    }

    pub(crate) fn status_at_query_composer(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" TAB ", Style::default().fg(palette::FG)),
            Span::styled("switch pane,  ", Style::default().fg(palette::FAINT)),
            Span::styled("UP/DOWN ", Style::default().fg(palette::FG)),
            Span::styled("navigate,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ENTER ", Style::default().fg(palette::FG)),
            Span::styled("select / expand,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'d' ", Style::default().fg(palette::FG)),
            Span::styled("details,  ", Style::default().fg(palette::FAINT)),
            Span::styled("ESC ", Style::default().fg(palette::FG)),
            Span::styled("close", Style::default().fg(palette::FAINT)),
        ]);
    }
}
