use super::{
    SysInspectUX,
    elements::{ActiveBox, DbListItem},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget,
    },
};

impl SysInspectUX {
    /// Render information box where data from the selected event is displayed
    fn _render_info_box(&self, rect: Rect, buf: &mut Buffer) {
        let title = "Action Data";
        let block = self._get_box_block(title, ActiveBox::Info);

        let header = Row::new(vec!["Key", "Value"]).style(Style::default().fg(Color::Yellow)).bottom_margin(1);
        let row1 = Row::new(vec!["foo", "bar"]);
        let row2 = Row::new(vec!["baz", "toto"]);
        let table = Table::new(vec![header, row1, row2], &[Constraint::Length(10), Constraint::Length(10)])
            .block(block)
            .column_spacing(1);
        Widget::render(table, rect, buf);
    }

    /// Render list of events
    fn _render_events_box(&self, rect: Rect, buf: &mut Buffer) {
        let title = "Action Results";
        let block = self._get_box_block(title, ActiveBox::Events);
        Widget::render(&block, rect, buf);

        let events_inner = block.inner(rect);
        let mut events_state = ListState::default();
        if !self.li_events.is_empty() {
            events_state.select(Some(self.selected_event));
        }

        StatefulWidget::render(
            self._wrap_list_items(self._get_list_items(&self.li_events, ActiveBox::Events), ActiveBox::Events),
            events_inner,
            buf,
            &mut events_state,
        );

        let mut events_scroll_state = ScrollbarState::default()
            .content_length(self.li_events.len())
            .position(if self.active_box == ActiveBox::Events { self.selected_event } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            events_inner,
            buf,
            &mut events_scroll_state,
        );
    }

    /// Render events block in the middle of the main screen
    fn _render_right_pane(&self, rect: Rect, buf: &mut Buffer) {
        let [model_events, event_data]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(15), Constraint::Min(10)])
            .split(rect)
            .as_ref()
            .try_into()
            .unwrap();

        self._render_events_box(model_events, buf);
        self._render_info_box(event_data, buf);
    }

    /// Render minions block in the middle of the main screen
    fn _render_minions_block(&self, rect: Rect, buf: &mut Buffer) {
        let title = "Minions";
        let block = self._get_box_block(title, ActiveBox::Minions);
        Widget::render(&block, rect, buf);

        let minions_inner = block.inner(rect);
        let mut minions_state = ListState::default();
        if !self.li_minions.is_empty() {
            minions_state.select(Some(self.selected_minion));
        }

        StatefulWidget::render(
            self._wrap_list_items(self._get_list_items(&self.li_minions, ActiveBox::Minions), ActiveBox::Minions),
            minions_inner,
            buf,
            &mut minions_state,
        );

        let mut minions_scroll_state = ScrollbarState::default()
            .content_length(self.li_minions.len())
            .position(if self.active_box == ActiveBox::Minions { self.selected_minion } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            minions_inner,
            buf,
            &mut minions_scroll_state,
        );
    }

    /// Prepares an active block with the border and title
    fn _get_box_block(&self, title: &str, hl: ActiveBox) -> Block {
        if self.active_box == hl {
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .title_style(Style::default().fg(Color::Black).bg(Color::LightGreen).add_modifier(Modifier::BOLD))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::LightGreen).bg(Color::Reset))
        } else {
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .title_style(Style::default().fg(Color::Gray).bg(Color::Reset))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Gray).bg(Color::Reset))
        }
    }

    /// Render cycles block in the middle of the main screen
    fn _render_cycles_block(&self, rect: Rect, buf: &mut Buffer) {
        let title = "Query Calls";
        let block = self._get_box_block(title, ActiveBox::Cycles);
        Widget::render(&block, rect, buf);

        let cycles_inner = block.inner(rect);
        let cycles = self.get_cycles();
        let mut list_state = ListState::default();
        if !cycles.is_empty() {
            list_state.select(Some(self.selected_cycle));
        }

        let items: Vec<ListItem> = self._get_list_items(&cycles, ActiveBox::Cycles);
        StatefulWidget::render(self._wrap_list_items(items, ActiveBox::Cycles), cycles_inner, buf, &mut list_state);

        let mut scroll_state = ScrollbarState::default().content_length(cycles.len()).position(self.selected_cycle);
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            cycles_inner,
            buf,
            &mut scroll_state,
        );
    }

    fn _get_list_items<T: DbListItem>(&self, items: &[T], hl: ActiveBox) -> Vec<ListItem<'static>> {
        items.iter().map(|item| ListItem::new(item.get_list_line(self.active_box != hl))).collect()
    }

    fn _wrap_list_items<'a>(&self, items: Vec<ListItem<'a>>, hl: ActiveBox) -> List<'a> {
        let hl_style = if self.active_box == hl {
            Style::default().fg(Color::White).bg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray).bg(Color::DarkGray)
        };
        List::new(items).highlight_style(hl_style)
    }
}

impl Widget for &SysInspectUX {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [cycles_a, minions_a, events_a]: [Rect; 3] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(30), Constraint::Min(0)])
            .split(area)
            .as_ref()
            .try_into()
            .unwrap();

        // Left/mid/right columns
        self._render_cycles_block(cycles_a, buf);
        self._render_minions_block(minions_a, buf);
        self._render_right_pane(events_a, buf);

        // Catch purge dialog
        self.dialog_purge(area, buf);
        self.dialog_exit(area, buf);
    }
}
