use super::{SysInspectUX, palette};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

impl SysInspectUX {
    pub(crate) fn status_at_cycles(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(palette::FG)),
            Span::styled("to select,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2192} ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2190} ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_minions(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(palette::FG)),
            Span::styled("to select,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2192} ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2190} ", Style::default().fg(palette::FG)),
            Span::styled("to query calls,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_action_results(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" Tab ", Style::default().fg(palette::FG)),
            Span::styled("to action data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Enter ", Style::default().fg(palette::FG)),
            Span::styled("to select an action data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2192} ", Style::default().fg(palette::FG)),
            Span::styled("to query calls,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2190} ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_action_data(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" Shift+Tab ", Style::default().fg(palette::FG)),
            Span::styled("to action results,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Up/Down ", Style::default().fg(palette::FG)),
            Span::styled("to scroll the data,  ", Style::default().fg(palette::FAINT)),
            Span::styled("\u{2190} ", Style::default().fg(palette::FG)),
            Span::styled("to minions,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("to quit,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'h' ", Style::default().fg(palette::FG)),
            Span::styled("for more help", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_minion_menu(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text =
            Line::from(vec![key("\u{2191}\u{2193} "), desc("navigate,  "), key("Enter "), desc("select,  "), key("Esc "), desc("close")]);
    }

    pub(crate) fn status_at_minions_browser(&mut self) {
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
            key("Esc "),
            desc("close"),
        ]);
    }

    pub(crate) fn status_at_minion_traits(&mut self) {
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
            key("Esc "),
            desc("back"),
        ]);
    }

    pub(crate) fn status_at_minion_logs(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        let mut spans = vec![
            key("\u{2191}\u{2193} "),
            desc("scroll,  "),
            key("PgUp/PgDn "),
            desc("skip,  "),
            key("Tab "),
            desc("filter,  "),
            key("/ "),
            desc("filter,  "),
            key("P "),
            desc(if self.minion_logs_polling { "pause,  " } else { "resume,  " }),
        ];
        if !self.minion_logs_polling {
            spans.push(key("R "));
            spans.push(desc("refresh,  "));
        }
        spans.push(key("Esc "));
        spans.push(desc("back"));
        self.status_text = Line::from(spans);
    }

    pub(crate) fn status_at_systop(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![
            key("↑↓ "),
            desc("select,  "),
            key("PgUp/PgDn "),
            desc("jump,  "),
            key("Enter "),
            desc("shootout,  "),
            key("/ "),
            desc("filter,  "),
            key("←→ "),
            desc("graph,  "),
            key("Tab/Shift+Tab "),
            desc("iface/fields,  "),
            key("c/m/p/n "),
            desc("sort,  "),
            key("Esc "),
            desc("close"),
        ]);
    }

    pub(crate) fn status_at_master_logs(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        let mut spans = vec![
            key("\u{2190}\u{2192} "),
            desc("switch tab,  "),
            key("\u{2191}\u{2193} "),
            desc("scroll,  "),
            key("PgUp/PgDn "),
            desc("skip,  "),
            key("Tab "),
            desc("filter,  "),
            key("/ "),
            desc("filter,  "),
            key("P "),
            desc(if self.master_logs_polling { "pause,  " } else { "resume,  " }),
        ];
        if !self.master_logs_polling {
            spans.push(key("R "));
            spans.push(desc("refresh,  "));
        }
        spans.push(key("Esc "));
        spans.push(desc("close"));
        self.status_text = Line::from(spans);
    }

    pub(crate) fn status_at_master_menu(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text =
            Line::from(vec![key("\u{2191}\u{2193} "), desc("navigate,  "), key("Enter "), desc("select,  "), key("Esc "), desc("close")]);
    }

    pub(crate) fn status_at_repo_manager(&mut self) {
        let p = &self.repo_manager.profiles;
        if p.detail_visible || p.create_visible || p.delete_visible || p.assign.visible {
            self.status_at_profiles();
            return;
        }
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![
            key("\u{2191}\u{2193} "),
            desc("navigate  "),
            key("Enter "),
            desc("info  "),
            key("Del "),
            desc("remove  "),
            key("Ins/i "),
            desc("add  "),
            key("L "),
            desc("libraries  "),
            key("Esc "),
            desc("close"),
        ]);
    }

    pub(crate) fn status_at_profiles(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));

        let p = &self.repo_manager.profiles;
        if p.delete_visible {
            self.status_text = Line::from(vec![key("Tab "), desc("switch  "), key("Enter "), desc("confirm  "), key("Esc "), desc("cancel")]);
        } else if p.create_visible {
            self.status_text = Line::from(vec![key("Tab "), desc("switch  "), key("Enter "), desc("create  "), key("Esc "), desc("cancel")]);
        } else if p.detail_visible {
            self.status_text = Line::from(vec![
                key("Tab "),
                desc("switch section  "),
                key("\u{2190}/\u{2192} "),
                desc("all modules/per model  "),
                key("+/- "),
                desc("expand/collapse  "),
                key("Enter "),
                desc("toggle  "),
                key("Esc "),
                desc("close"),
            ]);
        } else {
            self.status_text = Line::from(vec![
                key("\u{2191}\u{2193} "),
                desc("navigate  "),
                key("Enter "),
                desc("view/edit  "),
                key("Ins/n "),
                desc("create  "),
                key("Del "),
                desc("delete  "),
                key("Esc "),
                desc("close"),
            ]);
        }
    }

    pub(crate) fn status_at_query_composer(&mut self) {
        self.status_text = Line::from(vec![
            Span::styled(" Tab ", Style::default().fg(palette::FG)),
            Span::styled("switch pane,  ", Style::default().fg(palette::FAINT)),
            Span::styled("UP/DOWN ", Style::default().fg(palette::FG)),
            Span::styled("navigate,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Enter ", Style::default().fg(palette::FG)),
            Span::styled("select / expand,  ", Style::default().fg(palette::FAINT)),
            Span::styled("'d' ", Style::default().fg(palette::FG)),
            Span::styled("details,  ", Style::default().fg(palette::FAINT)),
            Span::styled("Esc ", Style::default().fg(palette::FG)),
            Span::styled("close", Style::default().fg(palette::FAINT)),
        ]);
    }

    pub(crate) fn status_at_registration_form(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![key("Tab "), desc("switch focus,  "), key("Enter "), desc("register,  "), key("Esc "), desc("cancel")]);
    }

    pub(crate) fn status_at_registration_progress(&mut self) {
        let key = |s| Span::styled(s, Style::default().fg(palette::FG));
        let desc = |s| Span::styled(s, Style::default().fg(palette::FAINT));
        self.status_text = Line::from(vec![key("Esc "), desc("cancel registration")]);
    }
}
