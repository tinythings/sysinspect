use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

/// Available actions in the minion context menu.
pub(crate) const MENU_ITEMS: &[&str] = &["System logs", "Defined traits"];

impl SysInspectUX {
    /// Render the minion actions context menu popup.
    ///
    /// Displays a list of available actions for the selected minion.
    /// The menu auto-sizes to the longest item and supports keyboard
    /// navigation with arrow keys, selection with Enter, and closing
    /// with Esc.
    pub fn minion_actions_menu(&self, parent: Rect, buf: &mut Buffer) {
        if !self.minions_menu_visible {
            return;
        }

        let host = self
            .minions_rows
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
            .unwrap_or_else(|| "unknown".to_string());

        let max_item_w = MENU_ITEMS.iter().map(|s| s.len()).max().unwrap_or(10);
        let title_segments = [" Actions on ".chars().count(), format!(" {host} ").chars().count()];
        let title_w = 1usize + title_segments.iter().sum::<usize>() + (title_segments.len().saturating_sub(1)) + 1usize;
        let inner_w = max_item_w.max(title_w) as u16;
        let w = (inner_w + 2).min(parent.width.saturating_sub(8)).max(20);
        let inner_h = (MENU_ITEMS.len() + 2) as u16;
        let h = (inner_h + 2).min(parent.height.saturating_sub(6)).max(5);
        let x = parent.x + (parent.width.saturating_sub(w)) / 2;
        let y = parent.y + (parent.height.saturating_sub(h)) / 2;
        let canvas = Rect { x, y, width: w, height: h };

        let bg = palette::POPUP_BG_BASE;
        Clear.render(canvas, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(palette::PROCESSING_GLOW))
            .style(Style::default().bg(bg));
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        title::overlay_gradient_title(
            buf,
            canvas,
            &title_style,
            &[
                TitleSegment { text: " Actions on ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG },
                TitleSegment { text: format!(" {host} "), bg: palette::PROCESSING_HEAT, fg: palette::SUCCESS_PEAK },
            ],
        );

        for (i, item) in MENU_ITEMS.iter().enumerate() {
            let row_y = inner.y + 1 + i as u16;
            if row_y >= inner.bottom() {
                break;
            }
            let style = if i == self.minions_menu_sel {
                Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT)
            } else {
                Style::default().fg(palette::FG).bg(bg)
            };
            let text = format!(" {:<width$} ", item, width = max_item_w);
            Paragraph::new(text).style(style).render(Rect { x: inner.x, y: row_y, width: inner.width, height: 1 }, buf);
        }

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
