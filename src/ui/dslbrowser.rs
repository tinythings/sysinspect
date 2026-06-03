use std::cell::Cell;

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};
use ratatui_cheese::input::{Input, InputState};

use super::SysInspectUX;

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
            DslFocus::Models,
            DslFocus::Target,
            DslFocus::State,
            DslFocus::ContextField(0),
            DslFocus::Minions,
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

    fn selected(&self) -> Option<usize> {
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
            context_fields: vec![
                ContextField { key: "Opt".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Foo".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Bar".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Etc".into(), value: String::new(), state: InputState::new() },
            ],
            focus: DslFocus::Query,
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
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        if ids.is_empty() {
            self.models = ListBox::new(vec!["(no models found)".to_string()], 0);
            self.model_data = vec![];
        } else {
            self.models = ListBox::new(ids, 0);
            self.model_data = rows;
        }
        self.catalog_diagnostics = failures;
        self.update_targets_and_states();
        self.visible = true;
        self.focus = DslFocus::Query;
    }

    fn update_targets_and_states(&mut self) {
        if let Some(row) = self.model_data.get(self.models.selected().unwrap_or(0)) {
            if row.entrypoints.is_empty() {
                self.targets = ListBox::new(vec!["(none)".to_string()], 0);
            } else {
                self.targets = ListBox::new(row.entrypoints.clone(), 0);
            }
            self.update_states_for_target();
        }
    }

    fn update_states_for_target(&mut self) {
        let model_idx = self.models.selected().unwrap_or(0);
        let target_id = self.targets.items.get(self.targets.selected().unwrap_or(0)).map(|s| s.as_str());
        if let (Some(row), Some(tid)) = (self.model_data.get(model_idx), target_id)
            && let Some((_, actions)) = row.target_actions.iter().find(|(id, _)| id == tid)
        {
            let mut states: Vec<String> = actions.iter().flat_map(|(_, s)| s.clone()).collect();
            states.sort();
            states.dedup();
            if !states.is_empty() {
                let display: Vec<String> = states.iter().map(|s| if s == "$" { "(default)".to_string() } else { s.clone() }).collect();
                self.states = ListBox::new(display, 0);
                return;
            }
        }
        // Fallback: global model states
        if let Some(row) = self.model_data.get(model_idx) {
            if row.states.is_empty() {
                self.states = ListBox::new(vec!["$".to_string()], 0);
            } else {
                let display: Vec<String> = row.states.iter().map(|s| if s == "$" { "(default)".to_string() } else { s.clone() }).collect();
                self.states = ListBox::new(display, 0);
            }
        }
    }

    fn s_fg() -> Style {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    }
    fn s_bd() -> Style {
        Self::s_fg().add_modifier(Modifier::BOLD)
    }
    fn s_di() -> Style {
        Style::default().fg(Color::Gray).bg(Color::DarkGray)
    }
    fn s_hl() -> Style {
        Style::default().fg(Color::Black).bg(Color::LightBlue)
    }
    fn s_hl_dim() -> Style {
        Style::default().fg(Color::Black).bg(Color::Gray)
    }
    fn s_bl() -> Style {
        Style::default().fg(Color::Cyan).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    }

    fn border_style(focus: DslFocus, current: DslFocus) -> Style {
        if current == focus { Style::default().fg(Color::White) } else { Style::default().fg(Color::Black) }
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
        let max_vis = 3usize;
        let trimmed = wrapped.len() > max_vis;
        let visible: Vec<&str> = wrapped.iter().take(max_vis).map(|s| s.as_str()).collect();
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
        self.render_description(rows[2], &visible, trimmed, buf);
        self.render_bottom(rows[3], buf);
    }

    fn build_target_description(&self) -> String {
        let model_idx = self.models.selected().unwrap_or(0);
        if matches!(self.focus, DslFocus::Target) || matches!(self.focus, DslFocus::State) {
            let target_id = self.targets.items.get(self.targets.selected().unwrap_or(0)).map(|s| s.as_str());
            let state_display = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
            let state_real = if state_display == "(default)" { "$" } else { state_display };
            if let (Some(row), Some(tid)) = (self.model_data.get(model_idx), target_id)
                && let Some((_, actions)) = row.target_actions.iter().find(|(id, _)| id == tid)
            {
                let descs: Vec<&str> =
                    actions.iter().filter(|(_, states)| states.iter().any(|s| s == state_real)).map(|(desc, _)| desc.as_str()).collect();
                if !descs.is_empty() {
                    if descs.len() == 1 {
                        return descs[0].to_string();
                    }
                    return descs.iter().enumerate().map(|(i, d)| format!("{}. {}", i + 1, d)).collect::<Vec<_>>().join("\n");
                }
            }
        }
        self.model_data.get(model_idx).map(|r| r.description.clone()).unwrap_or_default()
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
            if focused { Style::default().fg(Color::Black).bg(Color::DarkGray) } else { Style::default() }
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
                .track_symbol(Some("│"))
                .thumb_symbol("█")
                .render(Rect::new(sb_x, inner.y, 1, inner.height), buf, &mut sb_state);
        }
    }

    fn render_context_inline(&self, area: Rect, buf: &mut Buffer) {
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
            let inp = Input::new("").prompt("").placeholder(&field.key);
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

    fn render_description(&self, area: Rect, visible: &[&str], trimmed: bool, buf: &mut Buffer) {
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
        for (i, line) in visible.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let last = i == visible.len() - 1;
            if trimmed && last {
                let max_w = area.width.saturating_sub(2) as usize;
                let suffix = "… (press 'm' for more)";
                let cutoff = max_w.saturating_sub(suffix.chars().count());
                let display: String = line.chars().take(cutoff).collect();
                write_clipped(buf, area, area.x, y, &format!("  {display}"), Self::s_fg());
                write_clipped(
                    buf,
                    area,
                    area.x + 2 + display.chars().count() as u16,
                    y,
                    suffix,
                    Style::default().fg(Color::Cyan).bg(Color::DarkGray),
                );
            } else {
                write_clipped(buf, area, area.x, y, &format!("  {line}"), Self::s_fg());
            }
            y += 1;
        }
    }

    fn render_bottom(&self, area: Rect, buf: &mut Buffer) {
        let btn_y = area.y + 1;
        let call_lbl = format_button("Call");
        let close_lbl = format_button("Close");
        let total_w = (call_lbl.len() + close_lbl.len() + 1) as u16;
        let start_x = area.x + area.width.saturating_sub(total_w) / 2;
        let cs = if self.focus == DslFocus::Call { Self::s_hl() } else { Self::s_fg() };
        let xs = if self.focus == DslFocus::Close { Self::s_hl() } else { Self::s_fg() };
        Paragraph::new(call_lbl.clone()).style(cs).render(Rect::new(start_x, btn_y, call_lbl.len() as u16, 1), buf);
        Paragraph::new(close_lbl.clone()).style(xs).render(Rect::new(start_x + call_lbl.len() as u16 + 1, btn_y, close_lbl.len() as u16, 1), buf);
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.visible = false;
                return true;
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                return true;
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                return true;
            }
            _ => {}
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
                    if self.focus == DslFocus::Close {
                        self.visible = false;
                    }
                    return true;
                }
                false
            }
        }
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
            .title(" Query Composer ")
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(Color::Black).bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray))
            .style(Style::default().bg(Color::DarkGray));
        let inner = block.inner(popup);
        block.render(popup, buf);

        self.dsl_browser.render_content(inner, buf);

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
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
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
                    cell.set_bg(Color::Black);
                    cell.set_fg(Color::DarkGray);
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
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= max_width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
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
