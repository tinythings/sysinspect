use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use libsysinspect::console::ConsoleMinionInfoRow;
use ratatui::{
    layout::{Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_cheese::{
    input::{Input, InputState, InputStyles},
    tree::{Tree, TreeState, TreeStyles},
};

impl SysInspectUX {
    pub fn minion_traits(&self, parent: Rect, buf: &mut Buffer) {
        if !self.minions_visible || !self.minion_traits_visible {
            return;
        }

        let max_key = self.minion_traits_rows.iter().map(|r| r.key.len()).max().unwrap_or(4);
        let max_val = self.minion_traits_rows.iter().map(|r| Self::_info_value_str(&r.value).len()).max().unwrap_or(0);
        let line_w = (max_key + 4 + max_val + 2).max(10);
        let name = self
            .minion_traits_rows
            .first()
            .and_then(|r| if r.key == "hostname" { r.value.as_str().map(|s| s.to_string()) } else { None })
            .unwrap_or_else(|| {
                self.minions_rows
                    .iter()
                    .find(|r| {
                        let online: Vec<&libsysinspect::console::ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| r.alive).collect();
                        let offline: Vec<&libsysinspect::console::ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| !r.alive).collect();
                        match self.minions_focus {
                            1 => online.get(self.minions_online_sel).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                            2 => offline.get(self.minions_offline_sel).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                            _ => false,
                        }
                    })
                    .map(Self::online_host)
                    .unwrap_or_else(|| "unknown".to_string())
            });

        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        let title_segments = [
            TitleSegment { text: " Minion Traits: ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG, modifier: Modifier::empty() },
            TitleSegment { text: format!(" {name} "), bg: palette::PROCESSING_HEAT, fg: palette::FG, modifier: Modifier::empty() },
        ];
        let content_w = title::ensure_inner_width((line_w + 6) as u16, &title_style, &title_segments);
        let w = content_w.min(parent.width.saturating_sub(8)).max(40);
        let h = parent.height.saturating_sub(6).max(13);
        let x = if self.minions_focus == 1 { parent.x + parent.width.saturating_sub(w + 4) } else { parent.x + 4 };
        let y = parent.y + 3;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::PROCESSING_GLOW))
            .style(Style::default().bg(palette::POPUP_BG_BASE));

        let inner = block.inner(canvas);
        block.render(canvas, buf);

        title::overlay_gradient_title(buf, canvas, &title_style, &title_segments);

        if self.minion_traits_rows.is_empty() || inner.height < 4 {
            return;
        }

        let [filter_area, tree_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();

        Self::_render_info_filter(filter_area, buf, self.minion_traits_filter_focus, &self.minion_traits_filter);

        let f = self.minion_traits_filter.value().to_lowercase();
        let filtered_rows: Vec<ConsoleMinionInfoRow> =
            self.minion_traits_rows.iter().filter(|r| f.is_empty() || Self::_info_value_str(&r.value).to_lowercase().contains(&f)).cloned().collect();

        let groups = Self::build_info_tree(&filtered_rows);
        let n_groups = groups.len();
        let total_items = n_groups + filtered_rows.len();
        let filter_focused = self.minion_traits_filter_focus;
        let treesel = if filter_focused {
            Style::default().fg(palette::FG).bg(palette::POPUP_BG_BASE)
        } else {
            Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT)
        };
        let styles = TreeStyles {
            parent: Style::default().fg(palette::ACCENT).bg(palette::POPUP_BG_BASE).add_modifier(Modifier::BOLD),
            child: Style::default().fg(palette::FG).bg(palette::POPUP_BG_BASE).add_modifier(Modifier::BOLD),
            selected: treesel,
            chevron: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            chevron_active: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            chevron_dim: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            count: Style::default().fg(palette::GRAY_1).bg(palette::POPUP_BG_BASE),
            icon: Style::default().fg(palette::WARNING).bg(palette::POPUP_BG_BASE),
        };
        let tree = Tree::default().groups(groups).styles(styles).chevron_collapsed("▶ ").chevron_expanded("▼ ");

        let tree_inner = Rect::new(tree_area.x, tree_area.y, tree_area.width.saturating_sub(1), tree_area.height);

        if let Some(ref ts) = self.minion_traits_tree_state {
            let mut state = ts.clone();
            let (g, _) = state.selected();
            if g >= n_groups {
                state = TreeState::all_expanded(n_groups);
            }
            StatefulWidget::render(&tree, tree_inner, buf, &mut state);
        } else {
            let mut state = TreeState::all_expanded(n_groups);
            StatefulWidget::render(&tree, tree_inner, buf, &mut state);
        }

        let scroller_area = Rect::new(tree_area.right().saturating_sub(1), tree_area.y, 1, tree_area.height);
        let mut scroller = ScrollbarState::default()
            .content_length(total_items)
            .position(self.minion_traits_tree_state.as_ref().map(|ts| ts.selected().0).unwrap_or(0));
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(palette::BG_3))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(scroller_area, buf, &mut scroller);

        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        for idx in 0..w {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(h);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }
        for offset in 0..2u16 {
            for idx in 0..h {
                let sx = x.saturating_add(w).saturating_add(offset);
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

    fn _render_info_filter(area: Rect, buf: &mut Buffer, focused: bool, filter_state: &InputState) {
        let label_style =
            if focused { Style::default().fg(palette::FORM_LABEL).add_modifier(Modifier::BOLD) } else { Style::default().fg(palette::FORM_LABEL) };
        buf.set_string(area.x, area.y, "Filter: ", label_style);

        let input_x = area.x + 8u16;
        let input_w = area.width.saturating_sub(8);
        if input_w == 0 {
            return;
        }

        let field_bg = if focused { palette::HIGHLIGHT } else { palette::GRAY_1 };
        for cx in input_x..input_x + input_w {
            if let Some(cell) = buf.cell_mut(Position::new(cx, area.y)) {
                cell.set_bg(field_bg);
            }
        }

        let mut is = InputState::new();
        is.set_value(filter_state.value().to_string());
        is.set_focused(focused);
        let fc = filter_state.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        let styles = InputStyles { text: Style::default().fg(palette::BG_1), ..Default::default() };
        let inp = Input::new("").prompt("").placeholder("search values...").styles(styles);
        StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }
}
