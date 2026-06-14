use std::cell::Cell;

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};
use ratatui_cheese::input::{Input, InputState};
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::{dashed_title, gradient_rule};

use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DslFocus {
    Query,
    Minions,
    Models,
    Checkbook,
    Target,
    State,
    ContextField(usize),
    Call,
    Close,
}

impl DslFocus {
    fn tab_order() -> &'static [DslFocus] {
        &[
            DslFocus::Query,
            DslFocus::Minions,
            DslFocus::Models,
            DslFocus::Checkbook,
            DslFocus::Target,
            DslFocus::State,
            DslFocus::ContextField(0),
            DslFocus::Call,
            DslFocus::Close,
        ]
    }

    fn next(self) -> Self {
        let o = Self::tab_order();
        o[(o.iter().position(|f| *f == self).unwrap_or(0) + 1) % o.len()]
    }

    fn prev(self) -> Self {
        let o = Self::tab_order();
        o[(o.iter().position(|f| *f == self).unwrap_or(0) + o.len() - 1) % o.len()]
    }
}

#[derive(Debug)]
pub struct ContextField {
    pub key: String,
    pub value: String,
    pub desc: String,
    pub required: bool,
    pub state: InputState,
}

pub struct ListBox {
    pub items: Vec<String>,
    pub state: ListState,
    pub scroll: Cell<usize>,
}

impl std::fmt::Debug for ListBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListBox").field("items", &self.items).field("selected", &self.state.selected()).field("scroll", &self.scroll.get()).finish()
    }
}

impl ListBox {
    fn new(items: Vec<String>, selected: usize) -> Self {
        let mut s = ListState::default();
        s.select(Some(selected));
        Self { items, state: s, scroll: Cell::new(0) }
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    fn up(&mut self) {
        if let Some(cur) = self.state.selected()
            && cur > 0
        {
            self.state.select(Some(cur - 1));
            if cur - 1 < self.scroll.get() {
                self.scroll.set(self.scroll.get().saturating_sub(1));
            }
        }
    }

    fn down(&mut self) {
        if let Some(cur) = self.state.selected()
            && cur + 1 < self.items.len()
        {
            self.state.select(Some(cur + 1));
        }
    }
}

#[derive(Debug)]
pub struct DslBrowser {
    pub visible: bool,
    pub query: String,
    pub query_state: InputState,
    pub models: ListBox,
    pub states: ListBox,
    pub minions: ListBox,
    pub context_fields: Vec<ContextField>,
    pub focus: DslFocus,
    pub query_to_execute: Option<String>,
    pub call_requested: bool,
    pub desc_popup_visible: bool,
    pub desc_popup_text: String,
    pub desc_popup_scroll: usize,
    pub context_active: bool,
    pub error_required_key: Vec<String>,
    catalog_diagnostics: Vec<String>,
    model_data: Vec<libsysinspect::console::ConsoleModelRow>,
    all_minions: Vec<String>,
    pub checkbook_labels: ListBox,
    pub target_entities: ListBox,
}

impl DslBrowser {
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::from("*"),
            query_state: {
                let mut s = InputState::new();
                s.insert_char('*');
                s
            },
            models: ListBox::new(vec!["(press 'c' to load)".to_string()], 0),
            states: ListBox::new(vec!["$".to_string()], 0),
            minions: ListBox::new((1..=100).map(|i| format!("minion-{i:03}.example.net")).collect(), 0),
            context_fields: Vec::new(),
            focus: DslFocus::Query,
            query_to_execute: None,
            call_requested: false,
            desc_popup_visible: false,
            desc_popup_text: String::new(),
            desc_popup_scroll: 0,
            context_active: false,
            error_required_key: Vec::new(),
            catalog_diagnostics: Vec::new(),
            model_data: Vec::new(),
            all_minions: Vec::new(),
            checkbook_labels: ListBox::new(vec!["—".to_string()], 0),
            target_entities: ListBox::new(vec!["—".to_string()], 0),
        }
    }

    pub fn set_minions(&mut self, rows: Vec<String>) {
        self.all_minions = rows;
        self.filter_minions();
    }

    fn filter_minions(&mut self) {
        let q = self.query.trim();
        let q = if q.is_empty() { "*" } else { q };
        let filtered: Vec<String> = match glob::Pattern::new(q) {
            Ok(pat) => self.all_minions.iter().filter(|m| pat.matches(m)).cloned().collect(),
            Err(_) => self.all_minions.clone(),
        };
        if filtered.is_empty() && !self.all_minions.is_empty() {
            self.minions = ListBox::new(vec!["(no matches)".to_string()], 0);
        } else if filtered.is_empty() {
            self.minions = ListBox::new(vec!["(no minions)".to_string()], 0);
        } else {
            self.minions = ListBox::new(filtered, 0);
        }
    }

    pub fn load_models(&mut self, rows: Vec<libsysinspect::console::ConsoleModelRow>, failures: Vec<String>) {
        let rows: Vec<libsysinspect::console::ConsoleModelRow> = rows.into_iter().filter(|r| r.enabled).collect();
        let mut ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        if ids.is_empty() {
            ids = vec!["(no models found)".to_string()];
        } else {
            ids.insert(0, "(select)".to_string());
        }
        self.models = ListBox::new(ids, 0);
        self.model_data = rows;
        self.catalog_diagnostics = failures;
        self.context_active = false;
        self.context_fields = Vec::new();
        self.update_targets_and_states();
        self.visible = true;
        self.focus = DslFocus::Query;
    }

    fn update_targets_and_states(&mut self) {
        self.context_active = false;
        if let Some(row) = self.resolved_model() {
            let mut checkbook: Vec<String> = Vec::new();
            let mut entities: Vec<String> = Vec::new();
            for (i, entrypoint) in row.entrypoints.iter().enumerate() {
                let kind = row.entrypoint_kinds.get(i).map(|s| s.as_str()).unwrap_or("entity");
                if kind == "checkbook" {
                    checkbook.push(entrypoint.clone());
                } else {
                    entities.push(entrypoint.clone());
                }
            }
            if checkbook.is_empty() {
                checkbook = vec!["(none)".to_string()];
            } else {
                checkbook.insert(0, "(select)".to_string());
            }
            if entities.is_empty() {
                entities = vec!["(none)".to_string()];
            } else {
                entities.insert(0, "(select)".to_string());
            }
            self.checkbook_labels = ListBox::new(checkbook, 0);
            self.target_entities = ListBox::new(entities, 0);
            self.update_states_for_target();
        } else {
            self.checkbook_labels = ListBox::new(vec!["—".to_string()], 0);
            self.target_entities = ListBox::new(vec!["—".to_string()], 0);
            self.states = ListBox::new(vec!["$".to_string()], 0);
        }
    }

    fn update_states_for_target(&mut self) {
        let target_id = self.target_entities.items.get(self.target_entities.selected().unwrap_or(0)).cloned();
        let entry = self.resolved_model().and_then(|row| {
            target_id.as_deref().and_then(|tid| row.target_actions.iter().find(|(id, _)| id == tid).map(|(_, actions)| actions.clone()))
        });

        if let Some(actions) = entry
            && !actions.is_empty()
        {
            let mut states: Vec<String> = actions.iter().flat_map(|(_, s, _)| s.clone()).collect();
            states.sort();
            states.dedup();
            if !states.is_empty() {
                let display: Vec<String> = states.iter().map(|s| if s == "$" { "(default)".to_string() } else { s.clone() }).collect();
                self.states = ListBox::new(display, 0);
                self.update_context_fields(&actions);
                return;
            }
        }
        // Fallback
        self.context_active = false;
        self.context_fields = Vec::new();
        if let Some(row) = self.resolved_model() {
            if row.states.is_empty() {
                self.states = ListBox::new(vec!["$".to_string()], 0);
            } else {
                let display: Vec<String> = row.states.iter().map(|s| if s == "$" { "(default)".to_string() } else { s.clone() }).collect();
                self.states = ListBox::new(display, 0);
            }
        }
    }

    fn ctxfields_update(&mut self) {
        let target_id = self.target_entities.items.get(self.target_entities.selected().unwrap_or(0)).cloned();
        let entry = self.resolved_model().and_then(|row| {
            target_id.as_deref().and_then(|tid| row.target_actions.iter().find(|(id, _)| id == tid).map(|(_, actions)| actions.clone()))
        });

        if let Some(actions) = entry {
            self.update_context_fields(&actions);
        } else {
            self.context_active = false;
            self.context_fields = Vec::new();
        }
    }

    #[allow(clippy::type_complexity)]
    fn update_context_fields(&mut self, actions: &[(String, Vec<String>, Vec<(String, String, bool)>)]) {
        let state_display = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
        let state_real = if state_display == "(default)" { "$" } else { state_display };
        let mut ctx: Vec<(String, String, bool)> =
            actions.iter().filter(|(_, states, _)| states.iter().any(|s| s == state_real)).flat_map(|(_, _, ctx_vars)| ctx_vars.clone()).collect();
        ctx.sort_by(|a, b| a.0.cmp(&b.0));
        ctx.dedup_by(|a, b| a.0 == b.0);
        if !ctx.is_empty() {
            self.context_active = true;
            self.context_fields = ctx
                .into_iter()
                .map(|(k, d, r)| ContextField { key: k, value: String::new(), desc: d, required: r, state: InputState::new() })
                .collect();
        } else {
            self.context_active = false;
            self.context_fields = Vec::new();
        }
    }

    fn s_fg() -> Style {
        Style::default().fg(palette::FG)
    }
    fn s_bd() -> Style {
        Self::s_fg().add_modifier(Modifier::BOLD)
    }
    fn s_di() -> Style {
        Style::default().fg(palette::MUTED)
    }
    fn s_hl() -> Style {
        Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT)
    }
    fn s_hl_dim() -> Style {
        Style::default().fg(palette::HIGHLIGHT)
    }
    fn s_bl() -> Style {
        Style::default().fg(palette::PROCESSING).bg(palette::POPUP_BG_BASE)
    }
    fn s_fl() -> Style {
        Style::default().fg(palette::FORM_LABEL)
    }

    fn border_style(focus: DslFocus, current: DslFocus) -> Style {
        if current == focus { Style::default().fg(palette::ACCENT) } else { Style::default().fg(palette::FAINT) }
    }

    fn column_widths(area: Rect) -> (u16, u16) {
        let ctx_req = 28u16;
        let remaining = area.width.saturating_sub(ctx_req);
        let box_w = (remaining / 5).max(12);
        let ctx_w = area.width.saturating_sub(box_w * 5);
        (box_w, ctx_w)
    }

    pub fn render_content(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 12 {
            return;
        }

        let top_h = 1u16;
        let btn_h = 2u16;

        let desc_text = self.build_target_description();
        let wrapped = wrap_text(&desc_text, area.width.saturating_sub(4) as usize);
        let max_desc = 4usize;
        let visible: Vec<&str> = wrapped.iter().take(max_desc).map(|s| s.as_str()).collect();
        let has_more_desc = wrapped.len() > max_desc;
        let desc_h = 5u16; // rule + up to 4 lines of text

        let list_h = area.height.saturating_sub(top_h).saturating_sub(desc_h).saturating_sub(btn_h);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(top_h), Constraint::Length(list_h), Constraint::Length(desc_h), Constraint::Length(btn_h)])
            .split(area);

        let (box_w, ctx_w) = Self::column_widths(area);
        self.render_top(rows[0], box_w, ctx_w, buf);
        self.render_lists(rows[1], box_w, ctx_w, buf);
        self.render_description(rows[2], &visible, has_more_desc, buf);
        self.render_bottom(rows[3], buf);
    }

    fn build_target_description(&self) -> String {
        if (matches!(self.focus, DslFocus::Target) || matches!(self.focus, DslFocus::State))
            && let Some(desc) = self.build_target_description_unchecked()
        {
            return desc;
        }
        self.resolved_model().map(|r| r.description.clone()).unwrap_or_default()
    }

    fn build_target_description_unchecked(&self) -> Option<String> {
        let target_id = self.target_entities.items.get(self.target_entities.selected().unwrap_or(0)).map(|s| s.as_str())?;
        let state_display = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
        let state_real = if state_display == "(default)" { "$" } else { state_display };
        let row = self.resolved_model()?;
        let (_, actions) = row.target_actions.iter().find(|(id, _)| id == target_id)?;
        let descs: Vec<&str> =
            actions.iter().filter(|(_, states, _)| states.iter().any(|s| s == state_real)).map(|(desc, _, _)| desc.as_str()).collect();
        if descs.is_empty() {
            return None;
        }
        if descs.len() == 1 {
            Some(descs[0].to_string())
        } else {
            Some(descs.iter().enumerate().map(|(i, d)| format!("{}. {}", i + 1, d)).collect::<Vec<_>>().join("\n"))
        }
    }

    fn render_top(&self, area: Rect, box_w: u16, ctx_w: u16, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(ctx_w),
            ])
            .split(area);

        write_clipped(buf, chunks[0], chunks[0].x, chunks[0].y, "Query: ", Self::s_fl());
        let qf = self.focus == DslFocus::Query;
        let mut qs = InputState::new();
        qs.set_value(self.query.clone());
        qs.set_focused(qf);
        let qc = self.query_state.cursor_pos();
        while qs.cursor_pos() < qc {
            qs.move_right();
        }
        let inp = Input::new("").prompt("").placeholder("*");
        StatefulWidget::render(&inp, Rect::new(chunks[0].x + 7, chunks[0].y, chunks[0].width.saturating_sub(7), 1), buf, &mut qs);

        write_clipped(buf, chunks[1], chunks[1].x, chunks[1].y, " Models:", Self::s_fl());
        write_clipped(buf, chunks[2], chunks[2].x, chunks[2].y, " Checkbook:", Self::s_fl());
        write_clipped(buf, chunks[3], chunks[3].x, chunks[3].y, " Target:", Self::s_fl());
        write_clipped(buf, chunks[4], chunks[4].x, chunks[4].y, " State:", Self::s_fl());
        write_clipped(buf, chunks[5], chunks[5].x, chunks[5].y, " Context:", Self::s_fl());
    }

    fn render_lists(&self, area: Rect, box_w: u16, ctx_w: u16, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(ctx_w),
            ])
            .split(area);

        self.render_list_box(&self.minions, &chunks[0], DslFocus::Minions, buf);
        self.render_list_box(&self.models, &chunks[1], DslFocus::Models, buf);
        self.render_list_box(&self.checkbook_labels, &chunks[2], DslFocus::Checkbook, buf);
        self.render_list_box(&self.target_entities, &chunks[3], DslFocus::Target, buf);
        self.render_list_box(&self.states, &chunks[4], DslFocus::State, buf);
        self.render_context_inline(chunks[5], buf);
    }

    fn render_list_box(&self, lb: &ListBox, area: &Rect, target: DslFocus, buf: &mut Buffer) {
        let is_minions = matches!(target, DslFocus::Minions);
        let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Self::border_style(self.focus, target));
        let inner = block.inner(*area);
        block.render(*area, buf);

        let list_h = inner.height as usize;
        let total = lb.items.len();
        // Auto-scroll to keep selected visible
        if let Some(sel) = lb.state.selected() {
            let mut off = lb.scroll.get();
            if sel < off {
                off = sel;
            } else if sel >= off.saturating_add(list_h) {
                off = sel.saturating_sub(list_h.saturating_sub(1));
            }
            lb.scroll.set(off.min(total.saturating_sub(list_h)));
        }
        let offset = lb.scroll.get().min(total.saturating_sub(list_h));

        let visible: Vec<ListItem> = lb.items.iter().skip(offset).take(list_h).map(|s| ListItem::new(s.as_str())).collect();
        let focused = self.focus == target;
        let is_placeholder = lb.items.get(lb.state.selected().unwrap_or(0)).is_some_and(|s| s == "(select)");
        let hl = if is_placeholder && !focused {
            Style::default().fg(palette::MUTED).bg(palette::GRAY_0)
        } else if is_minions {
            if focused { Style::default().fg(palette::SECONDARY) } else { Style::default() }
        } else if focused {
            Self::s_hl()
        } else {
            Self::s_hl_dim()
        };
        let list = List::new(visible).highlight_style(hl);
        let mut ls = ListState::default();
        if focused {
            if let Some(sel) = lb.state.selected() {
                ls.select(Some(sel.saturating_sub(offset)));
            }
        } else if !is_minions && let Some(sel) = lb.state.selected() {
            ls.select(Some(sel.saturating_sub(offset)));
        }
        let list_area = Rect::new(inner.x, inner.y, inner.width.saturating_sub(2), inner.height);
        StatefulWidget::render(list, list_area, buf, &mut ls);

        if inner.width >= 2 && list_h > 0 {
            let sb_x = inner.right().saturating_sub(1);
            let mut sb_state = ScrollbarState::new(total).position(offset);
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{28FF}"))
                .thumb_symbol("█")
                .track_style(Style::default().bg(palette::BG_2))
                .thumb_style(Style::default().fg(palette::GRAY_1))
                .render(Rect::new(sb_x, inner.y, 1, inner.height), buf, &mut sb_state);
        }
    }

    fn render_context_inline(&self, area: Rect, buf: &mut Buffer) {
        if !self.context_active {
            buf.set_string(area.x + 1, area.y + 1, if self.resolved_model().is_some() { "No context defined" } else { " " }, Self::s_di());
            return;
        }
        let max_label_w = self.context_fields.iter().map(|f| f.key.len()).max().unwrap_or(4) as u16;
        let label_col_w = max_label_w + 2; // key:  plus padding
        let input_w = (area.width.saturating_sub(label_col_w)).max(15).min(area.width.saturating_sub(label_col_w));

        for (i, field) in self.context_fields.iter().enumerate() {
            let y = area.y + 1 + i as u16;
            if y >= area.bottom() {
                break;
            }
            let focused = matches!(self.focus, DslFocus::ContextField(idx) if idx == i);
            let req = if field.required { "*" } else { " " };
            let label = format!("{req}{:>width$}: ", field.key, width = max_label_w as usize);
            write_clipped(buf, area, area.x + 1, y, &label, Self::s_fl());
            let inp = Input::new("").prompt("").placeholder(if field.desc.is_empty() { &field.key } else { &field.desc });
            let mut is = InputState::new();
            is.set_value(field.value.clone());
            is.set_focused(focused);
            let fc = field.state.cursor_pos();
            while is.cursor_pos() < fc {
                is.move_right();
            }
            let ia = Rect::new(area.x + 1 + label_col_w, y, input_w, 1);
            StatefulWidget::render(&inp, ia, buf, &mut is);
        }
    }

    fn render_description(&self, area: Rect, visible: &[&str], has_more: bool, buf: &mut Buffer) {
        // Dash rule separator
        render_dash_rule(area, buf, has_more);
        let mut y = area.y + 1;
        let fail_count = self.catalog_diagnostics.len();
        if fail_count > 0 {
            write_clipped(buf, area, area.x, y, &format!(" {} failed model(s) ", fail_count), Self::s_di());
            y += 1;
        }
        if visible.is_empty() && fail_count == 0 {
            write_clipped(buf, area, area.x, y, " (no description)", Self::s_di());
            return;
        }
        for line in visible {
            if y >= area.bottom() {
                break;
            }
            write_clipped(buf, area, area.x, y, &format!("  {line}"), Self::s_fg());
            y += 1;
        }
    }

    fn render_bottom(&self, area: Rect, buf: &mut Buffer) {
        let btn_y = area.y + 1;
        let call_lbl = format_button("Call");
        let close_lbl = format_button("Close");
        let total_w = (call_lbl.len() + close_lbl.len() + 1) as u16;
        let start_x = area.x + area.width.saturating_sub(total_w) / 2;

        let b_sel = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD);
        let b_unsel = Style::default().fg(palette::FG).bg(palette::BG_2).add_modifier(Modifier::BOLD);

        let cs = if self.focus == DslFocus::Call { b_sel } else { b_unsel };
        let xs = if self.focus == DslFocus::Close { b_sel } else { b_unsel };
        Paragraph::new(call_lbl.clone()).style(cs).render(Rect::new(start_x, btn_y, call_lbl.len() as u16, 1), buf);
        Paragraph::new(close_lbl.clone()).style(xs).render(Rect::new(start_x + call_lbl.len() as u16 + 1, btn_y, close_lbl.len() as u16, 1), buf);
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                if self.desc_popup_visible {
                    self.desc_popup_visible = false;
                    return true;
                }
                self.visible = false;
                return true;
            }
            KeyCode::Tab => {
                let mut next = self.focus.next();
                if matches!(next, DslFocus::ContextField(_)) && !self.context_active {
                    next = DslFocus::Call;
                }
                self.focus = next;
                return true;
            }
            KeyCode::BackTab => {
                let mut prev = self.focus.prev();
                if matches!(prev, DslFocus::ContextField(_)) && !self.context_active {
                    prev = DslFocus::State;
                }
                self.focus = prev;
                return true;
            }
            KeyCode::Char('d') if !matches!(self.focus, DslFocus::Query | DslFocus::ContextField(_)) && self.resolved_model().is_some() => {
                self.desc_popup_text = self.build_full_description();
                self.desc_popup_scroll = 0;
                self.desc_popup_visible = true;
                return true;
            }
            _ => {}
        }

        if self.desc_popup_visible {
            match code {
                KeyCode::Up => {
                    self.desc_popup_scroll = self.desc_popup_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.desc_popup_scroll = self.desc_popup_scroll.saturating_add(1);
                }
                KeyCode::Enter | KeyCode::Esc => {
                    self.desc_popup_visible = false;
                }
                _ => {}
            }
            return true;
        }

        match self.focus {
            DslFocus::Query => {
                handle_query_edit(code, &mut self.query_state);
                self.query = self.query_state.value().to_string();
                self.filter_minions();
                true
            }
            DslFocus::Models => {
                handle_list_nav(code, &mut self.models);
                self.update_targets_and_states();
                true
            }
            DslFocus::Checkbook => {
                handle_list_nav(code, &mut self.checkbook_labels);
                true
            }
            DslFocus::Target => {
                handle_list_nav(code, &mut self.target_entities);
                self.update_states_for_target();
                true
            }
            DslFocus::State => {
                handle_list_nav(code, &mut self.states);
                self.ctxfields_update();
                true
            }
            DslFocus::Minions => {
                handle_list_nav(code, &mut self.minions);
                true
            }
            DslFocus::ContextField(idx) => {
                let n = self.context_fields.len();
                if idx < n {
                    let f = &mut self.context_fields[idx];
                    handle_ctx_edit(code, f, idx, n, &mut self.focus);
                }
                true
            }
            DslFocus::Call | DslFocus::Close => {
                if code == KeyCode::Enter {
                    if self.focus == DslFocus::Call {
                        let query = self.build_query();
                        if query.is_some() {
                            let missing: Vec<String> =
                                self.context_fields.iter().filter(|f| f.required && f.value.is_empty()).map(|f| f.key.clone()).collect();
                            if !missing.is_empty() {
                                self.call_requested = true;
                                self.query_to_execute = None;
                                self.error_required_key = missing;
                                return true;
                            }
                            self.query_to_execute = query;
                            self.call_requested = true;
                            self.visible = false;
                        } else {
                            self.call_requested = true;
                            self.query_to_execute = None;
                        }
                    } else {
                        self.visible = false;
                    }
                    return true;
                }
                false
            }
        }
    }

    fn model_data_index(&self) -> Option<usize> {
        let sel = self.models.selected().unwrap_or(0);
        if sel == 0 { None } else { Some(sel.saturating_sub(1)) }
    }

    fn resolved_model(&self) -> Option<&libsysinspect::console::ConsoleModelRow> {
        self.model_data_index().and_then(|i| self.model_data.get(i))
    }

    fn build_query(&self) -> Option<String> {
        let model = self.models.items.get(self.models.selected().unwrap_or(0))?;
        if model == "(select)" || model == "(no models found)" {
            return None;
        }
        if let Some(cb_sel) = self.checkbook_labels.selected()
            && cb_sel > 0
        {
            let label = self.checkbook_labels.items.get(cb_sel)?;
            if label == "(select)" || label == "(none)" || label == "—" {
                return None;
            }
            return Some(format!("{model}:{label}"));
        }
        let target = self.target_entities.items.get(self.target_entities.selected().unwrap_or(0))?;
        if target == "(select)" || target == "(none)" || target == "—" {
            return None;
        }
        let state_display = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
        let state = if state_display == "(default)" { "$" } else { state_display };
        Some(format!("{model}/{target}/{state}"))
    }

    pub fn build_context_json(&self) -> Option<String> {
        if !self.context_active {
            return None;
        }
        let filled: Vec<&ContextField> = self.context_fields.iter().filter(|f| !f.value.is_empty()).collect();
        if filled.is_empty() {
            return None;
        }
        let pairs: Vec<String> = filled.iter().map(|f| format!("{}:{}", f.key, f.value)).collect();
        Some(pairs.join(","))
    }

    fn build_full_description(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(row) = self.resolved_model() {
            parts.push(format!("Model: {}\n{}", row.id, row.description));
        }
        let target_id = self.target_entities.items.get(self.target_entities.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
        if target_id != "(select)"
            && target_id != "(none)"
            && target_id != "—"
            && !target_id.is_empty()
            && let Some(targets_desc) = self.build_target_description_unchecked()
        {
            parts.push(format!("Target \"{target_id}\":\n{targets_desc}"));
        }
        if self.context_active {
            let max_key = self.context_fields.iter().map(|f| f.key.len()).max().unwrap_or(4);
            let ctx_lines: Vec<String> = self
                .context_fields
                .iter()
                .map(|f| {
                    let req = if f.required { "required" } else { "optional" };
                    let key_padded = format!("{:>width$}", f.key, width = max_key);
                    if f.desc.is_empty() { format!("{key_padded}  {req}  Unknown field") } else { format!("{key_padded}  {req}  {}", f.desc) }
                })
                .collect();
            parts.push(format!("Context:\n{}", ctx_lines.join("\n")));
        }
        parts.join("\n\n")
    }

    pub fn take_query(&mut self) -> Option<String> {
        self.query_to_execute.take()
    }
}

impl SysInspectUX {
    pub fn dialog_dsl_browser(&self, parent: Rect, buf: &mut Buffer) {
        if !self.dsl_browser.visible {
            return;
        }

        let popup_w = (parent.width * 85 / 100).max(90).min(parent.width.saturating_sub(1).max(1));
        let popup_h = (parent.height * 85 / 100).max(24).min(parent.height.saturating_sub(1).max(1));
        let x = parent.x + (parent.width.saturating_sub(popup_w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(popup_h)) / 2;
        let popup = Rect { x, y, width: popup_w, height: popup_h };

        Clear.render(popup, buf);

        let grad_colors = blend_2d(popup.width as usize, popup.height as usize, 10.0, &[palette::GRAY_0, palette::BG_2] as &[ratatui::style::Color]);
        for row in 0..popup.height {
            for col in 0..popup.width {
                let idx = row as usize * popup.width as usize + col as usize;
                if let Some(cell) = buf.cell_mut(Position::new(popup.x + col, popup.y + row)) {
                    cell.set_bg(grad_colors[idx]);
                }
            }
        }

        let model_name = self.dsl_browser.models.items.get(self.dsl_browser.models.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
        let target_id = self.dsl_browser.target_entities.items.get(self.dsl_browser.target_entities.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
        let state_display = self.dsl_browser.states.items.get(self.dsl_browser.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");

        let has_model = !model_name.is_empty() && model_name != "(select)" && model_name != "(no models found)";
        let has_target = !target_id.is_empty() && target_id != "(select)" && target_id != "(none)" && target_id != "—";
        let has_state = !state_display.is_empty() && state_display != "(select)" && state_display != "(default)" && state_display != "$";

        let overshadowed = self.dsl_browser.desc_popup_visible;

        let (title_fg, glow_bg, heat_bg, peak_bg, proc_bg) = if overshadowed {
            (
                palette::MUTED,
                palette::PROCESSING_GLOW_DIMMED,
                palette::PROCESSING_HEAT_DIMMED,
                palette::PROCESSING_PEAK_DIMMED,
                palette::PROCESSING_DIMMED,
            )
        } else {
            (palette::FG, palette::PROCESSING_GLOW, palette::PROCESSING_HEAT, palette::PROCESSING_PEAK, palette::PROCESSING)
        };

        let mut title_segments = vec![TitleSegment { text: " Query Composer ".into(), bg: glow_bg, fg: title_fg, modifier: Modifier::empty() }];
        if has_model {
            title_segments.push(TitleSegment {
                text: format!(" {model_name} "),
                bg: heat_bg,
                fg: palette::SUCCESS_PEAK,
                modifier: Modifier::empty(),
            });
        }
        if has_target {
            title_segments.push(TitleSegment { text: format!(" {target_id} "), bg: peak_bg, fg: palette::BG_2, modifier: Modifier::empty() });
        }
        if has_state {
            title_segments.push(TitleSegment { text: format!(" {state_display} "), bg: proc_bg, fg: palette::BG_3, modifier: Modifier::empty() });
        }

        let border_color = glow_bg;
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(2))
            .style(Style::default());
        let inner = block.inner(popup);
        block.render(popup, buf);
        let title_style = TitleStyle::cyberpunk(border_color);
        title::overlay_gradient_title(buf, popup, &title_style, &title_segments);

        self.dsl_browser.render_content(inner, buf);

        if self.dsl_browser.desc_popup_visible {
            self.render_desc_popup(popup, buf);
        }

        // MS-DOS style shadows
        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        for idx in 0..popup_w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(popup_h);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }
        for offset in 0..2u16 {
            for idx in 0..popup_h {
                let sx = x.saturating_add(popup_w).saturating_add(offset);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > max_x || sy > max_y {
                    continue;
                }
                if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                    cell.set_bg(palette::SHADOW_BG);
                    cell.set_fg(palette::SHADOW_FG);
                }
            }
        }
    }
    fn render_desc_popup(&self, parent: Rect, buf: &mut Buffer) {
        let text = &self.dsl_browser.desc_popup_text;
        if text.is_empty() {
            return;
        }
        let w = (parent.width * 80 / 100).max(40).min(parent.width.saturating_sub(2));
        let text_h = (parent.height * 60 / 100).clamp(8, 20);
        let h = text_h.saturating_add(3);
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        let _bg = palette::POPUP_BG_1;
        Clear.render(canvas, buf);

        let grad_colors = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::BG_1, palette::BG_2] as &[ratatui::style::Color]);
        for row in 0..canvas.height {
            for col in 0..canvas.width {
                let idx = row as usize * canvas.width as usize + col as usize;
                if let Some(cell) = buf.cell_mut(Position::new(canvas.x + col, canvas.y + row)) {
                    cell.set_bg(grad_colors[idx]);
                }
            }
        }

        let model_name = self.dsl_browser.models.items.get(self.dsl_browser.models.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("?");
        let target_id = self.dsl_browser.target_entities.items.get(self.dsl_browser.target_entities.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
        let has_target = target_id != "(select)" && target_id != "(none)" && target_id != "—" && !target_id.is_empty();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::PROCESSING_GLOW))
            .style(Style::default());
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        let mut segments = vec![
            TitleSegment { text: " Details on ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG, modifier: Modifier::empty() },
            TitleSegment { text: format!(" {model_name} "), bg: palette::PROCESSING_HEAT, fg: palette::SUCCESS_PEAK, modifier: Modifier::empty() },
        ];
        if has_target {
            segments.push(TitleSegment {
                text: format!(" {target_id} "),
                bg: palette::PROCESSING_PEAK,
                fg: palette::SUCCESS_PEAK,
                modifier: Modifier::empty(),
            });
        }
        title::overlay_gradient_title(buf, canvas, &title_style, segments.as_slice());

        // Build rendered lines with section headers
        let body_style = Style::default().fg(palette::FG);

        // Layout: scrollable content area + close button
        let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        let content_area = chunks[0];
        let btn_area = chunks[1];

        let text_w = content_area.width.saturating_sub(1);

        let glamour_text_fg = palette::PROCESSING;
        let glamour_grad_start = palette::PRIMARY;
        let glamour_grad_end = palette::PROCESSING_DIMMED;

        // Compute total lines for scrollbar
        let mut total_lines = 0usize;
        let desc_max_w = text_w.saturating_sub(3) as usize;
        for line in text.split('\n') {
            if line.starts_with("Model: ") || line.starts_with("Target \"") || line.starts_with("Context:") || line.is_empty() {
                total_lines += 1;
            } else if line.contains("  required") || line.contains("  optional") {
                let tag = if line.contains("  required") { "required" } else { "optional" };
                let tag_pos = line.find(&format!("  {tag}")).unwrap_or(line.len());
                let key = &line[..tag_pos];
                let indent = "    ";
                let after_key = (indent.len() + key.len()) as u16;
                let desc_col = after_key + 2 + tag.len() as u16 + 2;
                let desc_width = text_w.saturating_sub(desc_col).max(10) as usize;
                let rest = &line[tag_pos + 2 + tag.len()..];
                let desc = rest.trim_start();
                if desc.is_empty() {
                    total_lines += 1;
                } else {
                    let wrapped = wrap_text(desc, desc_width);
                    total_lines += wrapped.len().max(1);
                }
            } else {
                let is_list = line.starts_with("- ") || (line.as_bytes().first().is_some_and(|c| c.is_ascii_digit()) && line.contains(". "));
                let indent = if is_list { "      " } else { "    " };
                total_lines += wrap_text(&format!("{indent}{line}"), desc_max_w).len().max(1);
            }
        }
        let max_off = total_lines.saturating_sub(content_area.height as usize);
        let offset = self.dsl_browser.desc_popup_scroll.min(max_off);

        let mut yy = content_area.y;
        let mut in_context = false;
        for line in text.split('\n').skip(offset) {
            if yy >= content_area.bottom() {
                break;
            }
            if let Some(model_name) = line.strip_prefix("Model: ") {
                in_context = false;
                let prefix = " Model \"";
                let suffix = "\" ";
                let cx = content_area.x;
                buf.set_string(cx, yy, prefix, Style::default().fg(glamour_text_fg));
                let name_x = cx + prefix.len() as u16;
                buf.set_string(name_x, yy, model_name, Style::default().fg(palette::PRIMARY));
                let fill_x = name_x + model_name.len() as u16;
                buf.set_string(fill_x, yy, suffix, Style::default().fg(glamour_text_fg));
                gradient_rule(
                    Rect { x: content_area.x, y: yy, width: text_w, height: 1 },
                    buf,
                    fill_x + suffix.len() as u16,
                    glamour_grad_start,
                    glamour_grad_end,
                );
                yy += 1;
            } else if line.starts_with("Target \"") {
                in_context = false;
                let prefix_end = line.find(':').unwrap_or(line.len());
                let title_line = &line[..prefix_end];
                if let Some(quote_open) = title_line.find('"')
                    && let Some(quote_close) = title_line[quote_open + 1..].find('"')
                {
                    let before = format!(" {}", &title_line[..=quote_open]);
                    let target_name = &title_line[quote_open + 1..quote_open + 1 + quote_close];
                    let after = "\" ";
                    let cx = content_area.x;
                    buf.set_string(cx, yy, &before, Style::default().fg(glamour_text_fg));
                    let name_x = cx + before.len() as u16;
                    buf.set_string(name_x, yy, target_name, Style::default().fg(palette::PRIMARY));
                    let fill_x = name_x + target_name.len() as u16;
                    buf.set_string(fill_x, yy, after, Style::default().fg(glamour_text_fg));
                    gradient_rule(
                        Rect { x: content_area.x, y: yy, width: text_w, height: 1 },
                        buf,
                        fill_x + after.len() as u16,
                        glamour_grad_start,
                        glamour_grad_end,
                    );
                    yy += 1;
                    continue;
                }
                dashed_title(
                    Rect { x: content_area.x, y: yy, width: text_w, height: 1 },
                    buf,
                    title_line,
                    glamour_text_fg,
                    glamour_grad_start,
                    glamour_grad_end,
                );
                yy += 1;
            } else if line.starts_with("Context:") {
                in_context = true;
                dashed_title(
                    Rect { x: content_area.x, y: yy, width: text_w, height: 1 },
                    buf,
                    "Context",
                    glamour_text_fg,
                    glamour_grad_start,
                    glamour_grad_end,
                );
                yy += 1;
            } else if line.is_empty() {
                yy += 1;
            } else if in_context && (line.contains("  required") || line.contains("  optional")) {
                let indent = "    ";
                let tag = if line.contains("  required") { "required" } else { "optional" };
                let tag_pos = line.find(&format!("  {tag}")).unwrap_or(line.len());
                let key = &line[..tag_pos];
                let rest = &line[tag_pos + 2 + tag.len()..];
                let tag_color = if tag == "required" { palette::WARNING_PEAK } else { palette::MUTED };
                let desc = rest.trim_start();
                if yy < content_area.bottom() {
                    let after_key = (indent.len() + key.len()) as u16;
                    let desc_col = content_area.x + after_key + 2 + tag.len() as u16 + 2;
                    let desc_width = content_area.right().saturating_sub(1).saturating_sub(desc_col).max(10) as usize;
                    let dstyle = if desc == "Unknown field" { Style::default().fg(palette::FAINT) } else { body_style };

                    if desc.is_empty() {
                        buf.set_string(content_area.x, yy, format!("{indent}{key}"), Style::default().fg(palette::ACCENT));
                        buf.set_string(content_area.x + after_key, yy, format!("  {tag}"), Style::default().fg(tag_color));
                        yy += 1;
                    } else {
                        let wrapped = wrap_text(desc, desc_width);
                        let first_line = format!("{indent}{key}  {tag}  {}", wrapped[0]);
                        buf.set_string(content_area.x, yy, &first_line, body_style);
                        buf.set_string(content_area.x, yy, format!("{indent}{key}"), Style::default().fg(palette::ACCENT));
                        buf.set_string(content_area.x + after_key, yy, format!("  {tag}"), Style::default().fg(tag_color));
                        yy += 1;
                        for cont in &wrapped[1..] {
                            if yy >= content_area.bottom() {
                                break;
                            }
                            buf.set_string(desc_col, yy, cont, dstyle);
                            yy += 1;
                        }
                    }
                }
            } else {
                let is_list = line.starts_with("- ") || (line.as_bytes().first().is_some_and(|c| c.is_ascii_digit()) && line.contains(". "));
                let indent = if is_list { "      " } else { "    " };
                for wrapped in wrap_text(&format!("{indent}{line}"), (text_w.saturating_sub(3)) as usize) {
                    if yy >= content_area.bottom() {
                        break;
                    }
                    buf.set_string(content_area.x, yy, &wrapped, body_style);
                    yy += 1;
                }
            }
        }

        let sb_x = content_area.right().saturating_sub(1);
        let mut sb_state = ScrollbarState::new(total_lines).position(offset);
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_2))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(Rect::new(sb_x, content_area.y, 1, content_area.height), buf, &mut sb_state);

        let close = format_button("Close");
        let close_w = close.len() as u16;
        let btn_x = btn_area.x + (btn_area.width.saturating_sub(close_w)) / 2;
        Paragraph::new(close)
            .style(Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT).add_modifier(Modifier::BOLD))
            .render(Rect::new(btn_x, btn_area.y, close_w, 1), buf);

        // MS-DOS shadow
        let ba = buf.area();
        let mx = ba.right().saturating_sub(1);
        let my = ba.bottom().saturating_sub(1);
        for idx in 0..w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(h);
            if sx > mx || sy > my {
                continue;
            }
            if let Some(c) = buf.cell_mut(Position::new(sx, sy)) {
                c.set_bg(palette::SHADOW_BG);
                c.set_fg(palette::SHADOW_FG);
            }
        }
        for off in 0..2u16 {
            for idx in 0..h {
                let sx = x.saturating_add(w).saturating_add(off);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > mx || sy > my {
                    continue;
                }
                if let Some(c) = buf.cell_mut(Position::new(sx, sy)) {
                    c.set_bg(palette::SHADOW_BG);
                    c.set_fg(palette::SHADOW_FG);
                }
            }
        }
    }
}

fn render_dash_rule(area: Rect, buf: &mut Buffer, has_more: bool) {
    let w = area.width as usize;
    if w < 6 {
        return;
    }
    let suffix = if has_more { " ('d' for more)" } else { "" };
    let label = format!(" Description{suffix} ");
    let fill_w = w.saturating_sub(label.len()) / 2;
    let fill = "\u{2500}".repeat(fill_w);
    let line = format!("{fill}{label}{fill}");
    buf.set_string(area.x, area.y, &line, Style::default().fg(palette::PROCESSING));
}

fn format_button(label: &str) -> String {
    let t: String = label.chars().take(10).collect();
    let pad = 14usize.saturating_sub(t.chars().count());
    let l = pad / 2;
    format!("[{}{}{}]", " ".repeat(l), t, " ".repeat(pad - l))
}

fn handle_query_edit(code: KeyCode, qs: &mut InputState) -> bool {
    match code {
        KeyCode::Char(c) => {
            qs.insert_char(c);
            true
        }
        KeyCode::Backspace => {
            qs.delete_before();
            true
        }
        KeyCode::Delete => {
            qs.delete_at();
            true
        }
        KeyCode::Left => {
            qs.move_left();
            true
        }
        KeyCode::Right => {
            qs.move_right();
            true
        }
        KeyCode::Home => {
            qs.home();
            true
        }
        KeyCode::End => {
            qs.end();
            true
        }
        _ => false,
    }
}

fn handle_list_nav(code: KeyCode, lb: &mut ListBox) -> bool {
    match code {
        KeyCode::Up => {
            lb.up();
            true
        }
        KeyCode::Down => {
            lb.down();
            true
        }
        KeyCode::PageUp => {
            for _ in 0..10 {
                lb.up();
            }
            true
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                lb.down();
            }
            true
        }
        _ => false,
    }
}

fn handle_ctx_edit(code: KeyCode, f: &mut ContextField, idx: usize, total: usize, focus: &mut DslFocus) -> bool {
    match code {
        KeyCode::Up => {
            if idx > 0 {
                *focus = DslFocus::ContextField(idx - 1);
            }
            true
        }
        KeyCode::Down => {
            if idx + 1 < total {
                *focus = DslFocus::ContextField(idx + 1);
            }
            true
        }
        KeyCode::Tab => {
            *focus = if idx + 1 < total { DslFocus::ContextField(idx + 1) } else { DslFocus::Minions };
            true
        }
        KeyCode::BackTab => {
            *focus = if idx > 0 { DslFocus::ContextField(idx - 1) } else { DslFocus::State };
            true
        }
        KeyCode::Char(c) => {
            f.state.insert_char(c);
            f.value = f.state.value().to_string();
            true
        }
        KeyCode::Backspace => {
            f.state.delete_before();
            f.value = f.state.value().to_string();
            true
        }
        KeyCode::Delete => {
            f.state.delete_at();
            f.value = f.state.value().to_string();
            true
        }
        KeyCode::Left => {
            f.state.move_left();
            true
        }
        KeyCode::Right => {
            f.state.move_right();
            true
        }
        KeyCode::Home => {
            f.state.home();
            true
        }
        KeyCode::End => {
            f.state.end();
            true
        }
        _ => false,
    }
}

pub(crate) fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() || max_width < 4 {
        return vec![];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }
        let lead = &paragraph[..paragraph.len() - paragraph.trim_start().len()];
        let mut current = lead.to_string();
        for word in trimmed.split_whitespace() {
            if current.len() + 1 + word.len() > max_width {
                lines.push(std::mem::take(&mut current));
                current = lead.to_string();
            }
            if !current.is_empty() && current != lead {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

fn write_clipped(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    if area.width == 0 || area.height == 0 || y < area.y || y >= area.bottom() || x < area.x || x >= area.right() {
        return;
    }

    let max_chars = area.right().saturating_sub(x) as usize;
    if max_chars == 0 {
        return;
    }

    let clipped: String = text.chars().take(max_chars).collect();
    if clipped.is_empty() {
        return;
    }

    buf.set_string(x, y, clipped, style);
}
