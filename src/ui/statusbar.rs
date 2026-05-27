use super::SysInspectUX;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

impl SysInspectUX {
    /// Set static status when cycles pan is active
    pub(crate) fn status_at_cycles(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" ENTER ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to select, ", Style::default().fg(Color::LightCyan)),
            Span::styled("RIGHT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to minions, ", Style::default().fg(Color::LightCyan)),
            Span::styled("LEFT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to action results, ", Style::default().fg(Color::LightCyan)),
            Span::styled("ESC ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to quit, ", Style::default().fg(Color::LightCyan)),
            Span::styled("'h' ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("for more help", Style::default().fg(Color::LightCyan)),
        ]);
    }

    /// Set static status when minions pan is active
    pub(crate) fn status_at_minions(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" ENTER ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to select, ", Style::default().fg(Color::LightCyan)),
            Span::styled("RIGHT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to action results, ", Style::default().fg(Color::LightCyan)),
            Span::styled("LEFT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to query calls, ", Style::default().fg(Color::LightCyan)),
            Span::styled("ESC ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to quit, ", Style::default().fg(Color::LightCyan)),
            Span::styled("'h' ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("for more help", Style::default().fg(Color::LightCyan)),
        ]);
    }

    /// Set static status when action results pan is active
    pub(crate) fn status_at_action_results(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" TAB ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to action data, ", Style::default().fg(Color::LightCyan)),
            Span::styled("ENTER ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to select an action data, ", Style::default().fg(Color::LightCyan)),
            Span::styled("RIGHT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to query calls, ", Style::default().fg(Color::LightCyan)),
            Span::styled("LEFT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to minions, ", Style::default().fg(Color::LightCyan)),
            Span::styled("ESC ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to quit, ", Style::default().fg(Color::LightCyan)),
            Span::styled("'h' ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("for more help", Style::default().fg(Color::LightCyan)),
        ]);
    }

    /// Set static status when cycles pan is active
    pub(crate) fn status_at_action_data(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" SHIFT+TAB ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to action results, ", Style::default().fg(Color::LightCyan)),
            Span::styled("Up/Down ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to scroll the data, ", Style::default().fg(Color::LightCyan)),
            Span::styled("LEFT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to minions, ", Style::default().fg(Color::LightCyan)),
            Span::styled("ESC ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("to quit, ", Style::default().fg(Color::LightCyan)),
            Span::styled("'h' ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("for more help", Style::default().fg(Color::LightCyan)),
        ]);
    }

    /// Set status when online minions popup is active
    pub(crate) fn status_at_online_minions(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        let desc = |s| Span::styled(s, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD));
        self.status_text = Line::from(vec![
            key("TAB "),
            desc("cycle focus, "),
            key("SHIFT+TAB "),
            desc("reverse, "),
            key("↑↓ "),
            desc("navigate, "),
            key("←→ "),
            desc("tree fold, "),
            key("Enter "),
            desc("select/toggle, "),
            key("PgUp/PgDn "),
            desc("skip rows, "),
            key("'t' "),
            desc("tag trait, "),
            key("ESC "),
            desc("close"),
        ]);
    }
}
