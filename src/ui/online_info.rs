use super::{SysInspectUX, palette};
use ratatui::{
    layout::{Alignment, Position},
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_cheese::tree::{Tree, TreeState, TreeStyles};

impl SysInspectUX {
    pub fn dialog_online_minion_info(&self, parent: Rect, buf: &mut Buffer) {
        if !self.online_minions_visible || !self.online_minions_info_visible {
            return;
        }

        let w = parent.width.saturating_sub(8).max(40);
        let h = parent.height.saturating_sub(6).max(12);
        let x = parent.x + 4;
        let y = parent.y + 3;
        let canvas = Rect { x, y, width: w, height: h };

        Clear.render(canvas, buf);

        let name = self
            .online_minions_info_rows
            .first()
            .and_then(|r| if r.key == "hostname" { r.value.as_str().map(|s| s.to_string()) } else { None })
            .unwrap_or_else(|| {
                self.online_minions_rows
                    .iter()
                    .find(|r| {
                        let filtered_online = self.online_minions_rows.iter().filter(|r| r.alive).collect::<Vec<_>>();
                        let filtered_offline = self.online_minions_rows.iter().filter(|r| !r.alive).collect::<Vec<_>>();
                        match self.online_minions_focus {
                            1 => filtered_online.get(self.online_minions_online_selected).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                            2 => filtered_offline.get(self.online_minions_offline_selected).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                            _ => false,
                        }
                    })
                    .map(Self::online_host)
                    .unwrap_or_else(|| "unknown".to_string())
            });

        let t = format!(" Minion Info: {name} ");
        let block = Block::default()
            .title(Line::from(vec![
                Span::styled("\u{E0B2}", Style::default().fg(palette::BORDER)),
                Span::styled(t, Style::default().fg(palette::BLACK).bg(palette::BORDER).add_modifier(Modifier::BOLD)),
                Span::styled("\u{E0B0}", Style::default().fg(palette::BORDER)),
            ]))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(palette::BORDER).bg(palette::POPUP_BG_BASE))
            .style(Style::default().bg(palette::POPUP_BG_BASE));

        let inner = block.inner(canvas);
        block.render(canvas, buf);

        if self.online_minions_info_rows.is_empty() {
            return;
        }

        let groups = Self::build_info_tree(&self.online_minions_info_rows);
        let n_groups = groups.len();
        let total_items = n_groups + self.online_minions_info_rows.len();
        let styles = TreeStyles {
            parent: Style::default().fg(palette::ACCENT).bg(palette::POPUP_BG_BASE).add_modifier(Modifier::BOLD),
            child: Style::default().fg(palette::FG).bg(palette::POPUP_BG_BASE).add_modifier(Modifier::BOLD),
            selected: Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT),
            chevron: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            chevron_active: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            chevron_dim: Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE),
            count: Style::default().fg(palette::GRAY_1).bg(palette::POPUP_BG_BASE),
            icon: Style::default().fg(palette::WARNING).bg(palette::POPUP_BG_BASE),
        };
        let tree = Tree::default().groups(groups).styles(styles).chevron_collapsed("▶ ").chevron_expanded("▼ ");

        let tree_area = Rect::new(inner.x, inner.y, inner.width.saturating_sub(1), inner.height);

        if let Some(ref ts) = self.online_minions_tree_state {
            let mut state = ts.clone();
            StatefulWidget::render(&tree, tree_area, buf, &mut state);
        } else {
            let mut state = TreeState::all_expanded(n_groups);
            StatefulWidget::render(&tree, tree_area, buf, &mut state);
        }

        let scroller_area = Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height);
        let mut scroller = ScrollbarState::default()
            .content_length(total_items)
            .position(self.online_minions_tree_state.as_ref().map(|ts| ts.selected().0).unwrap_or(0));
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
}
