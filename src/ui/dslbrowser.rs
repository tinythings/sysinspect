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

use std::cell::Cell;

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
            models: ListBox::new((1..=20).map(|i| format!("model-{i:02}")).collect(), 0),
            targets: ListBox::new((1..=15).map(|i| format!("target-{i:02}")).collect(), 0),
            states: ListBox::new((1..=12).map(|i| format!("state-{i:02}")).collect(), 0),
            minions: ListBox::new((1..=100).map(|i| format!("minion-{i:03}.example.net")).collect(), 0),
            context_fields: vec![
                ContextField { key: "Opt".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Foo".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Bar".into(), value: String::new(), state: InputState::new() },
                ContextField { key: "Etc".into(), value: String::new(), state: InputState::new() },
            ],
            focus: DslFocus::Query,
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
        let list_h = area.height.saturating_sub(top_h + 4);
        let bot_h = area.height.saturating_sub(top_h + list_h);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(top_h), Constraint::Length(list_h), Constraint::Length(bot_h)])
            .split(area);

        let (box_w, ctx_w) = Self::column_widths(area);
        self.render_top(rows[0], box_w, ctx_w, buf);
        self.render_lists(rows[1], box_w, ctx_w, buf);
        self.render_bottom(rows[2], buf);
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

        buf.set_string(chunks[0].x, chunks[0].y, "Query: ", Self::s_bd());
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

        buf.set_string(chunks[1].x, chunks[1].y, " Models:", Self::s_bl());
        buf.set_string(chunks[2].x, chunks[2].y, " Target:", Self::s_bl());
        buf.set_string(chunks[3].x, chunks[3].y, " State:", Self::s_bl());
        buf.set_string(chunks[4].x, chunks[4].y, " Context:", Self::s_bl());
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

        // Real ratatui Scrollbar
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

    fn render_context_inline(&self, area: Rect, buf: &mut Buffer) {
        let max_label_w = self.context_fields.iter().map(|f| f.key.len()).max().unwrap_or(4) as u16;
        let label_col_w = max_label_w + 2; // key:  plus padding
        let input_w = (area.width.saturating_sub(label_col_w)).max(15);

        for (i, field) in self.context_fields.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let focused = matches!(self.focus, DslFocus::ContextField(idx) if idx == i);
            let label = format!("{:>width$}: ", field.key, width = max_label_w as usize);
            buf.set_string(area.x, y, &label, Self::s_bd());
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

    fn render_bottom(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        buf.set_string(chunks[0].x, chunks[0].y, " Diagnostics: 0 warnings, 0 errors ", Self::s_di());

        let m = self.models.items.get(self.models.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("?");
        let e = self.targets.items.get(self.targets.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("?");
        let s = self.states.items.get(self.states.selected().unwrap_or(0)).map(|s| s.as_str()).unwrap_or("$");
        buf.set_string(chunks[1].x, chunks[1].y, format!("  Preview: model={m}  target={e}  state={s}"), Self::s_fg());

        let btn_y = chunks[3].y;
        let call_lbl = format_button("Call");
        let close_lbl = format_button("Close");
        let total_w = (call_lbl.len() + close_lbl.len() + 1) as u16;
        let start_x = chunks[3].x + chunks[3].width.saturating_sub(total_w) / 2;

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
                true
            }
            DslFocus::Models => {
                handle_list_nav(code, &mut self.models);
                true
            }
            DslFocus::Target => {
                handle_list_nav(code, &mut self.targets);
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

        let popup_w = (parent.width * 85 / 100).max(90);
        let popup_h = (parent.height * 85 / 100).max(24);
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

        // MS-DOS style shadows (copied from alert.rs)
        for idx in 0..popup_w {
            let cell = buf.cell_mut(Position::new(x + 2 + idx, y + popup_h)).unwrap();
            cell.set_bg(Color::Black);
            cell.set_fg(Color::DarkGray);
        }
        for offset in 0..2u16 {
            for idx in 0..popup_h {
                let cell = buf.cell_mut(Position::new(x + popup_w + offset, y + idx + 1)).unwrap();
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
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
