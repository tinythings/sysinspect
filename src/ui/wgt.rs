use super::{
    SysInspectUX, UISizes,
    elements::{ActiveBox, DbListItem, EventListItem},
    palette, typecolors,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget,
    },
};

impl SysInspectUX {
    /// Render information box where data from the selected event is displayed
    fn _render_info_box(&self, rect: Rect, buf: &mut Buffer) {
        let csize = self.size.get();
        self.size.set(UISizes { table_info: rect.height.saturating_sub(2) as usize, ..csize });

        let block = self._get_box_block("Action Data", ActiveBox::Info);
        Widget::render(&block, rect, buf);
        let inner = block.inner(rect);

        let rule_fill = Style::default().fg(palette::PROCESSING_BASE);
        let rule_title = Style::default().fg(palette::PROCESSING).add_modifier(Modifier::BOLD);

        let evt = match self.get_selected_event() {
            Some(eli) => eli,
            None => {
                Widget::render(Paragraph::new("N/A").style(Style::default().fg(palette::ERROR)), inner, buf);
                return;
            }
        };
        let info_rows = evt.get_event_table(15);

        let mut info_rows_ref = self.info_rows.borrow_mut();
        info_rows_ref.clear();
        for (k, v) in &self.event_data {
            if k.starts_with("data.") {
                let k = k.strip_prefix("data.").unwrap_or_default().to_string();
                info_rows_ref.push(Row::new(vec![EventListItem::yc(format!("{k}:"), 15), Cell::from(typecolors::format_typed_value(v))]));
            }
        }
        let has_details = !info_rows_ref.is_empty();

        if has_details {
            let [gen_title_area, gen_table_area] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(info_rows.len() as u16)])
                .split(inner)
                .as_ref()
                .try_into()
                .unwrap();
            render_rule_line(gen_title_area, buf, "General", rule_title, rule_fill);
            Widget::render(Table::new(info_rows, &[Constraint::Length(15), Constraint::Min(0)]).column_spacing(1), gen_table_area, buf);

            let [_, det_title_area, det_content_area]: [Rect; 3] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
                .split(Rect::new(inner.x, gen_table_area.bottom(), inner.width, inner.bottom().saturating_sub(gen_table_area.bottom())))
                .as_ref()
                .try_into()
                .unwrap();
            render_rule_line(det_title_area, buf, "Details", rule_title, rule_fill);

            let ex_nfo_parts =
                Layout::default().direction(Direction::Horizontal).constraints([Constraint::Min(0), Constraint::Length(1)]).split(det_content_area);
            let ex_nfo_area = ex_nfo_parts[0];
            let ex_nfo_scroller = ex_nfo_parts[1];

            let end = (2 + self.actdt_info_offset + det_content_area.height.saturating_sub(2) as usize).min(info_rows_ref.len());
            let displayed = &info_rows_ref[self.actdt_info_offset..end];
            Widget::render(Table::new(displayed.to_vec(), &[Constraint::Length(15), Constraint::Min(0)]).column_spacing(1), ex_nfo_area, buf);

            let mut scroller_state = ScrollbarState::default().content_length(info_rows_ref.len()).position(self.actdt_info_offset);
            Scrollbar::default()
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{28FF}"))
                .thumb_symbol("█")
                .track_style(Style::default().bg(palette::BG_2))
                .thumb_style(Style::default().fg(palette::GRAY_1))
                .render(ex_nfo_scroller, buf, &mut scroller_state);
        } else {
            let [gen_title_area, gen_table_area] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner)
                .as_ref()
                .try_into()
                .unwrap();
            render_rule_line(gen_title_area, buf, "General", rule_title, rule_fill);
            Widget::render(Table::new(info_rows, &[Constraint::Length(15), Constraint::Min(0)]).column_spacing(1), gen_table_area, buf);
        }
    }

    /// Render list of events
    fn _render_events_box(&self, rect: Rect, buf: &mut Buffer) {
        let csize = self.size.get();
        self.size.set(UISizes { table_events: rect.height.saturating_sub(2) as usize, ..csize });

        let title = "Action Results";
        let block = self._get_box_block(title, ActiveBox::Events);
        Widget::render(&block, rect, buf);

        let events_inner = block.inner(rect);
        let mut events_state = ListState::default();
        if !self.li_events.is_empty() {
            events_state.select(Some(self.selected_event));
        }

        let left_w = self.li_events.iter().map(|e| e.left_width()).max().unwrap_or(0);
        let items: Vec<ListItem> = self.li_events.iter().map(|e| ListItem::new(e.get_aligned_line(left_w))).collect();
        let hl_style = if self.main_box_active(ActiveBox::Events) {
            Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::MUTED).bg(palette::SURFACE)
        };
        StatefulWidget::render(List::new(items).highlight_style(hl_style), events_inner, buf, &mut events_state);

        let mut events_scroll_state = ScrollbarState::default()
            .content_length(self.li_events.len())
            .position(if self.main_box_active(ActiveBox::Events) { self.selected_event } else { 0 });
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_2))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(events_inner, buf, &mut events_scroll_state);
    }

    /// Render events block in the middle of the main screen
    fn _render_right_pane(&self, rect: Rect, buf: &mut Buffer) {
        let [model_events, event_data]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 4), Constraint::Min(0)])
            .split(rect)
            .as_ref()
            .try_into()
            .unwrap();

        self._render_events_box(model_events, buf);
        self._render_info_box(event_data, buf);
    }

    /// Render minions block in the middle of the main screen
    fn _render_minions_block(&self, rect: Rect, buf: &mut Buffer) {
        let csize = self.size.get();
        self.size.set(UISizes { table_minions: rect.height.saturating_sub(2) as usize, ..csize });

        let title = if !self.li_minions.is_empty() { format!("Minions ({})", self.li_minions.len()) } else { "Minions".to_string() };
        let block = self._get_box_block(&title, ActiveBox::Minions);
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
            .position(if self.main_box_active(ActiveBox::Minions) { self.selected_minion } else { 0 });
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_2))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(minions_inner, buf, &mut minions_scroll_state);
    }

    /// Prepares an active block with the border and title
    fn _get_box_block(&self, title: &str, hl: ActiveBox) -> Block<'_> {
        if self.main_box_active(hl) {
            let t = title.to_string();
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::styled("\u{E0B2}", Style::default().fg(palette::ACCENT)),
                    Span::styled(t, Style::default().fg(palette::BLACK).bg(palette::ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled("\u{E0B0}", Style::default().fg(palette::ACCENT)),
                ]))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::ACCENT))
        } else {
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .title_style(Style::default().fg(palette::MUTED))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::FAINT))
        }
    }

    /// Render cycles block in the middle of the main screen
    fn _render_cycles_block(&self, rect: Rect, buf: &mut Buffer) {
        let csize = self.size.get();
        self.size.set(UISizes { table_cycles: rect.height.saturating_sub(2) as usize, ..csize });

        let title = format!("Query Calls ({})", self.cycles_buf.len());
        let block = self._get_box_block(&title, ActiveBox::Cycles);
        Widget::render(&block, rect, buf);

        let cycles_inner = block.inner(rect);
        let cycles = self.cycles_buf.clone();
        let mut list_state = ListState::default();
        if !cycles.is_empty() {
            list_state.select(Some(self.selected_cycle));
        }

        let items: Vec<ListItem> = self._get_list_items(&cycles, ActiveBox::Cycles);
        StatefulWidget::render(self._wrap_list_items(items, ActiveBox::Cycles), cycles_inner, buf, &mut list_state);

        let mut scroll_state = ScrollbarState::default().content_length(cycles.len()).position(self.selected_cycle);
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_2))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(cycles_inner, buf, &mut scroll_state);
    }

    fn _get_list_items<T: DbListItem>(&self, items: &[T], hl: ActiveBox) -> Vec<ListItem<'static>> {
        items.iter().map(|item| ListItem::new(item.get_list_line(!self.main_box_active(hl)))).collect()
    }

    fn _wrap_list_items<'a>(&self, items: Vec<ListItem<'a>>, hl: ActiveBox) -> List<'a> {
        let hl_style = if self.main_box_active(hl) {
            Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::MUTED).bg(palette::SURFACE)
        };
        List::new(items).highlight_style(hl_style)
    }
}

/// Render a decorated rule line: ` Title ////////////////////////////////`
/// with one leading space and dash fill to end of area, minus one trailing space.
pub(crate) fn render_rule_line(area: Rect, buf: &mut Buffer, title: &str, title_style: Style, fill_style: Style) {
    if area.width < 6 {
        return;
    }
    let label = format!(" {title} ");
    let label_w = label.len() as u16;
    buf.set_string(area.x, area.y, &label, title_style);

    let fill_start = area.x.saturating_add(label_w);
    let fill_end = area.right().saturating_sub(1);
    for x in fill_start..fill_end.min(fill_start.saturating_add(area.width)) {
        buf.set_string(x, area.y, "/", fill_style);
    }
}

impl Widget for &SysInspectUX {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        // Fill main background
        Block::default().style(Style::default().bg(palette::BG_1)).render(area, buf);

        let cycles_max = self.cycles_buf.iter().map(|c| c.get_list_line(false).width()).max().unwrap_or(10);
        let minions_max = self.li_minions.iter().map(|m| m.get_list_line(false).width()).max().unwrap_or(8);

        let cycles_w = (cycles_max as u16 + 5).max(20).min(area.width.saturating_sub(20));
        let minions_w = (minions_max as u16 + 5).max(20).min(area.width.saturating_sub(cycles_w).saturating_sub(10));

        let [cycles_a, minions_a, events_a]: [Rect; 3] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(cycles_w), Constraint::Length(minions_w), Constraint::Min(0)])
            .split(area)
            .as_ref()
            .try_into()
            .unwrap();

        // Left/mid/right columns
        self._render_cycles_block(cycles_a, buf);
        self._render_minions_block(minions_a, buf);
        self._render_right_pane(events_a, buf);

        // Catch dialogs
        self.dialog_purge(area, buf);
        self.dialog_exit(area, buf);
        self.dialog_help(area, buf);
        self.dialog_minions(area, buf);
        self.minion_actions_menu(area, buf);
        self.minion_traits(area, buf);
        self.dialog_minion_logs(area, buf);
        self.dialog_trait_tag(area, buf);
        self.dialog_cluster_confirm(area, buf);
        self.dialog_dsl_browser(area, buf);
        self.dialog_error(area, buf);
    }
}
