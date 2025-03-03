use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget,
    },
};

use super::{SysInspectUX, elements::ActiveBox};

impl Widget for &SysInspectUX {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [cycles_a, minions_a, events_a]: [Rect; 3] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Length(30), Constraint::Min(0)])
            .split(area)
            .as_ref()
            .try_into()
            .unwrap();

        //----------------------------------
        // Left column: Cycles
        //----------------------------------
        let cycles_block = if self.active_box == ActiveBox::Cycles {
            Block::default()
                .borders(Borders::ALL)
                .title("Cycles")
                .title_style(Style::default().fg(Color::LightYellow))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
        } else {
            Block::default()
                .borders(Borders::ALL)
                .title("Cycles")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
        };
        Widget::render(&cycles_block, cycles_a, buf);

        let cycles_inner = cycles_block.inner(cycles_a);
        let cycles = self.get_cycles();
        let items: Vec<ListItem> = cycles
            .iter()
            .map(|cycle| {
                let line = Line::from(vec![
                    Span::styled(">", Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(cycle.get_title(), Style::default().fg(Color::LightYellow)), // title in light yellow
                ]);
                ListItem::new(line)
            })
            .collect();
        let mut list_state = ListState::default();
        if !cycles.is_empty() {
            list_state.select(Some(self.selected_cycle));
        }

        let cycles_list =
            List::new(items).highlight_style(Style::default().fg(Color::White).bg(Color::Green).add_modifier(Modifier::BOLD));

        StatefulWidget::render(cycles_list, cycles_inner, buf, &mut list_state);

        let mut cycles_scroll_state = ScrollbarState::default().content_length(cycles.len()).position(self.selected_cycle);
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            cycles_inner,
            buf,
            &mut cycles_scroll_state,
        );

        //----------------------------------
        // Middle column: Minions
        //----------------------------------
        let minions_block = if self.active_box == ActiveBox::Minions {
            Block::default().borders(Borders::ALL).title("Minions").border_type(BorderType::Double)
        } else {
            Block::default().borders(Borders::ALL).title("Minions")
        };
        Widget::render(&minions_block, minions_a, buf);

        let minions_inner = minions_block.inner(minions_a);
        let minion_items: Vec<ListItem> = self.minions.iter().map(|m| ListItem::new(m.as_str())).collect();
        let mut minions_state = ListState::default();
        if !self.minions.is_empty() {
            minions_state.select(Some(self.selected_minion));
        }
        let minions_list = List::new(minion_items)
            .highlight_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        StatefulWidget::render(minions_list, minions_inner, buf, &mut minions_state);

        let mut minions_scroll_state = ScrollbarState::default()
            .content_length(self.minions.len())
            .position(if self.active_box == ActiveBox::Minions { self.selected_minion } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            minions_inner,
            buf,
            &mut minions_scroll_state,
        );

        //----------------------------------
        // Right column: Some table
        //----------------------------------
        let [model_events, event_data]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(events_a)
            .as_ref()
            .try_into()
            .unwrap();

        let right_block = if self.active_box == ActiveBox::Events {
            Block::default().borders(Borders::ALL).title("Events").border_type(BorderType::Double)
        } else {
            Block::default().borders(Borders::ALL).title("Right Block")
        };
        Widget::render(&right_block, model_events, buf);

        let events_inner = right_block.inner(model_events);
        let events_items: Vec<ListItem> = self.events.iter().map(|m| ListItem::new(m.as_str())).collect();
        let mut events_state = ListState::default();
        if !self.events.is_empty() {
            events_state.select(Some(self.selected_event));
        }
        let events_list = List::new(events_items)
            .highlight_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        StatefulWidget::render(events_list, events_inner, buf, &mut events_state);

        let mut events_scroll_state = ScrollbarState::default()
            .content_length(self.events.len())
            .position(if self.active_box == ActiveBox::Events { self.selected_event } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            events_inner,
            buf,
            &mut events_scroll_state,
        );

        let bottom_block = Block::default().borders(Borders::ALL).title("Bottom Right");

        let header = Row::new(vec!["Key", "Value"]).style(Style::default().fg(Color::Yellow)).bottom_margin(1);
        let row1 = Row::new(vec!["foo", "bar"]);
        let row2 = Row::new(vec!["baz", "toto"]);
        let table = Table::new(vec![header, row1, row2], &[Constraint::Length(10), Constraint::Length(10)])
            .block(bottom_block)
            .column_spacing(1);
        Widget::render(table, event_data, buf);

        // Catch purge dialog
        self.dialog_purge(area, buf);
        self.dialog_exit(area, buf);
    }
}
