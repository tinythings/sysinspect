use std::cell::Cell;

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};
use ratatui_cheese::input::{Input, InputState};

use super::{SysInspectUX, palette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DslFocus {
    Query,
    Minions,
    Models,
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
    pub targets: ListBox,
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
    catalog_diagnostics: Vec<String>,
    model_data: Vec<libsysinspect::console::ConsoleModelRow>,
    all_minions: Vec<String>,
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
            targets: ListBox::new(vec!["—".to_string()], 0),
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
            catalog_diagnostics: Vec::new(),
            model_data: Vec::new(),
            all_minions: Vec::new(),
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
            let mut targets = row.entrypoints.clone();
            if targets.is_empty() {
                targets = vec!["(none)".to_string()];
            } else {
                targets.insert(0, "(select)".to_string());
            }
            self.targets = ListBox::new(targets, 0);
            self.update_states_for_target();
        } else {
            self.targets = ListBox::new(vec!["—".to_string()], 0);
            self.states = ListBox::new(vec!["$".to_string()], 0);
        }
    }

    fn update_states_for_target(&mut self) {
        let target_id = self.targets.items.get(self.targets.selected().unwrap_or(0)).cloned();
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
        if let Some(row) = self.resolved_model() {
            if row.states.is_empty() {
                self.states = ListBox::new(vec!["$".to_string()], 0);
            } else {
                let display: Vec<String> = row.states.iter().map(|s| if s == "$" { "(default)".to_string() } else { s.clone() }).collect();
                self.states = ListBox::new(display, 0);
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn update_context_fields(&mut self, actions: &[(String, Vec<String>, Vec<(String, String)>)]) {
        let state_display = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
        let state_real = if state_display == "(default)" { "$" } else { state_display };
        let mut ctx: Vec<(String, String)> =
            actions.iter().filter(|(_, states, _)| states.iter().any(|s| s == state_real)).flat_map(|(_, _, ctx_vars)| ctx_vars.clone()).collect();
        ctx.sort_by(|a, b| a.0.cmp(&b.0));
        ctx.dedup_by(|a, b| a.0 == b.0);
        if !ctx.is_empty() {
            self.context_active = true;
            self.context_fields =
                ctx.into_iter().map(|(k, d)| ContextField { key: k, value: String::new(), desc: d, state: InputState::new() }).collect();
        } else {
            self.context_fields = Vec::new();
        }
    }

    fn s_fg() -> Style {
        Style::default().fg(palette::FG).bg(palette::POPUP_BG_BASE)
    }
    fn s_bd() -> Style {
        Self::s_fg().add_modifier(Modifier::BOLD)
    }
    fn s_di() -> Style {
        Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE)
    }
    fn s_hl() -> Style {
        Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT)
    }
    fn s_hl_dim() -> Style {
        Style::default().fg(palette::BLACK).bg(palette::SURFACE)
    }
    fn s_bl() -> Style {
        Style::default().fg(palette::PROCESSING).bg(palette::POPUP_BG_BASE).add_modifier(Modifier::BOLD)
    }

    fn border_style(focus: DslFocus, current: DslFocus) -> Style {
        if current == focus { Style::default().fg(palette::ACCENT) } else { Style::default().fg(palette::FAINT) }
    }

    fn column_widths(area: Rect) -> (u16, u16) {
        let ctx_req = 28u16;
        let remaining = area.width.saturating_sub(ctx_req);
        let box_w = (remaining / 4).max(16);
        let ctx_w = area.width.saturating_sub(box_w * 4);
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
        let max_desc = 3usize;
        let visible: Vec<&str> = wrapped.iter().take(max_desc).map(|s| s.as_str()).collect();
        let has_header = !self.catalog_diagnostics.is_empty() || visible.is_empty();
        let desc_h = (visible.len() as u16).saturating_add(if has_header { 1 } else { 0 });

        let list_h = area.height.saturating_sub(top_h).saturating_sub(desc_h).saturating_sub(btn_h);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(top_h), Constraint::Length(list_h), Constraint::Length(desc_h), Constraint::Length(btn_h)])
            .split(area);

        let (box_w, ctx_w) = Self::column_widths(area);
        self.render_top(rows[0], box_w, ctx_w, buf);
        self.render_lists(rows[1], box_w, ctx_w, buf);
        self.render_description(rows[2], &visible, buf);
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
        let target_id = self.targets.items.get(self.targets.selected().unwrap_or(0)).map(|s| s.as_str())?;
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
                Constraint::Length(ctx_w),
            ])
            .split(area);

        write_clipped(buf, chunks[0], chunks[0].x, chunks[0].y, "Query: ", Self::s_bd());
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

        write_clipped(buf, chunks[1], chunks[1].x, chunks[1].y, " Models:", Self::s_bl());
        write_clipped(buf, chunks[2], chunks[2].x, chunks[2].y, " Target:", Self::s_bl());
        write_clipped(buf, chunks[3], chunks[3].x, chunks[3].y, " State:", Self::s_bl());
        write_clipped(buf, chunks[4], chunks[4].x, chunks[4].y, " Context:", Self::s_bl());
    }

    fn render_lists(&self, area: Rect, box_w: u16, ctx_w: u16, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(box_w),
                Constraint::Length(ctx_w),
            ])
            .split(area);

        self.render_list_box(&self.minions, &chunks[0], DslFocus::Minions, buf);
        self.render_list_box(&self.models, &chunks[1], DslFocus::Models, buf);
        self.render_list_box(&self.targets, &chunks[2], DslFocus::Target, buf);
        self.render_list_box(&self.states, &chunks[3], DslFocus::State, buf);
        self.render_context_inline(chunks[4], buf);
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
        let hl = if is_minions {
            if focused { Style::default().fg(palette::BLACK).bg(palette::SURFACE) } else { Style::default() }
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
            buf.set_string(area.x, area.y, if self.resolved_model().is_some() { "No context defined" } else { " " }, Self::s_di());
            return;
        }
        let max_label_w = self.context_fields.iter().map(|f| f.key.len()).max().unwrap_or(4) as u16;
        let label_col_w = max_label_w + 2; // key:  plus padding
        let input_w = (area.width.saturating_sub(label_col_w)).max(15).min(area.width.saturating_sub(label_col_w));

        for (i, field) in self.context_fields.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let focused = matches!(self.focus, DslFocus::ContextField(idx) if idx == i);
            let label = format!("{:>width$}: ", field.key, width = max_label_w as usize);
            write_clipped(buf, area, area.x, y, &label, Self::s_bd());
            let inp = Input::new("").prompt("").placeholder(if field.desc.is_empty() { &field.key } else { &field.desc });
            let mut is = InputState::new();
            is.set_value(field.value.clone());
            is.set_focused(focused);
            let fc = field.state.cursor_pos();
            while is.cursor_pos() < fc {
                is.move_right();
            }
            let ia = Rect::new(area.x + label_col_w, y, input_w, 1);
            StatefulWidget::render(&inp, ia, buf, &mut is);
        }
    }

    fn render_description(&self, area: Rect, visible: &[&str], buf: &mut Buffer) {
        let mut y = area.y;
        let fail_count = self.catalog_diagnostics.len();
        let header = if fail_count > 0 {
            format!(" {} failed model(s) ", fail_count)
        } else if visible.is_empty() {
            " (no description) ".to_string()
        } else {
            String::new()
        };
        if !header.is_empty() {
            write_clipped(buf, area, area.x, y, &header, Self::s_di());
            y += 1;
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
            KeyCode::Char('m') => {
                if self.resolved_model().is_some() {
                    self.desc_popup_text = self.build_full_description();
                    self.desc_popup_scroll = 0;
                    self.desc_popup_visible = true;
                }
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
            DslFocus::Target => {
                handle_list_nav(code, &mut self.targets);
                self.update_states_for_target();
                true
            }
            DslFocus::State => {
                handle_list_nav(code, &mut self.states);
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
        let target = self.targets.items.get(self.targets.selected().unwrap_or(0))?;
        if model == "(select)" || model == "(no models found)" || target == "(select)" || target == "(none)" || target == "—" {
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
        let target_id = self.targets.items.get(self.targets.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("");
        if target_id != "(select)"
            && target_id != "(none)"
            && target_id != "—"
            && !target_id.is_empty()
            && let Some(targets_desc) = self.build_target_description_unchecked()
        {
            parts.push(format!("Target \"{target_id}\":\n{targets_desc}"));
        }
        if self.context_active {
            let has_desc = self.context_fields.iter().any(|f| !f.desc.is_empty());
            let ctx_text = if has_desc {
                self.context_fields
                    .iter()
                    .map(|f| if f.desc.is_empty() { f.key.clone() } else { format!("{} — {}", f.key, f.desc) })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                let keys: Vec<String> = self.context_fields.iter().enumerate().map(|(i, f)| format!("{}. {}", i + 1, f.key)).collect();
                format!("Required context keys (no descriptions available):\n{}", keys.join("\n"))
            };
            parts.push(format!("Context:\n{ctx_text}"));
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

        let block = Block::default()
            .title(Line::from(vec![
                Span::styled("\u{E0B2}", Style::default().fg(palette::BORDER)),
                Span::styled("Query Composer", Style::default().fg(palette::BLACK).bg(palette::BORDER)),
                Span::styled("\u{E0B0}", Style::default().fg(palette::BORDER)),
            ]))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(palette::BORDER).bg(palette::POPUP_BG_BASE))
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(palette::POPUP_BG_BASE));
        let inner = block.inner(popup);
        block.render(popup, buf);

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

        let bg = palette::POPUP_BG_1;
        Clear.render(canvas, buf);
        let block = Block::default()
            .title(Line::from(vec![
                Span::styled("\u{E0B2}", Style::default().fg(palette::FAINT)),
                Span::styled("Details", Style::default().fg(palette::FG).bg(palette::FAINT)),
                Span::styled("\u{E0B0}", Style::default().fg(palette::FAINT)),
            ]))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(palette::FAINT).bg(bg))
            .style(Style::default().bg(bg));
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        // Build rendered lines with section headers
        let body_style = Style::default().fg(palette::FG).bg(bg);

        // Layout: scrollable content area + close button
        let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        let content_area = chunks[0];
        let btn_area = chunks[1];

        let rule_s = Style::default().fg(palette::PROCESSING).add_modifier(Modifier::BOLD);
        let rule_f = Style::default().fg(palette::PROCESSING_BASE);

        // Compute total lines for scrollbar
        let mut total_lines = 0usize;
        for line in text.split('\n') {
            if line.starts_with("Model: ") || line.starts_with("Target \"") || line.starts_with("Context:") || line.is_empty() {
                total_lines += 1;
            } else {
                let is_list = line.starts_with("- ") || (line.as_bytes().first().is_some_and(|c| c.is_ascii_digit()) && line.contains(". "));
                let indent = if is_list { "      " } else { "    " };
                total_lines += wrap_text(&format!("{indent}{line}"), (content_area.width.saturating_sub(3)) as usize).len().max(1);
            }
        }
        let max_off = total_lines.saturating_sub(content_area.height as usize);
        let offset = self.dsl_browser.desc_popup_scroll.min(max_off);

        let mut yy = content_area.y;
        for line in text.split('\n').skip(offset) {
            if yy >= content_area.bottom() {
                break;
            }
            if let Some(title) = line.strip_prefix("Model: ") {
                super::wgt::render_rule_line(Rect { x: content_area.x, y: yy, width: content_area.width, height: 1 }, buf, title, rule_s, rule_f);
                yy += 1;
            } else if line.starts_with("Target \"") {
                if let Some(colon) = line.find(':') {
                    let title = &line[..colon];
                    super::wgt::render_rule_line(Rect { x: content_area.x, y: yy, width: content_area.width, height: 1 }, buf, title, rule_s, rule_f);
                    yy += 1;
                }
            } else if line.starts_with("Context:") {
                super::wgt::render_rule_line(Rect { x: content_area.x, y: yy, width: content_area.width, height: 1 }, buf, "Context", rule_s, rule_f);
                yy += 1;
            } else if line.is_empty() {
                yy += 1;
            } else {
                let is_list = line.starts_with("- ") || (line.as_bytes().first().is_some_and(|c| c.is_ascii_digit()) && line.contains(". "));
                let indent = if is_list { "      " } else { "    " };
                for wrapped in wrap_text(&format!("{indent}{line}"), (content_area.width.saturating_sub(3)) as usize) {
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

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
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
