use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use ratatui_glamour::rule::dashed_title;

struct MenuSection {
    title: &'static str,
    items: &'static [(&'static str, char)],
}

const MENU_SECTIONS: &[MenuSection] = &[
    MenuSection { title: "Tools", items: &[("System logs", 'L'), ("Defined traits", 'T')] },
    MenuSection { title: "Minion Operations", items: &[("Remote start", 'S'), ("Shutdown minion", 'D'), ("Force re-connect", 'F')] },
    MenuSection { title: "Cluster Operations", items: &[("Shutdown everything", 'X'), ("Reconnect all minions", 'A')] },
];

pub(crate) fn total_menu_items() -> usize {
    MENU_SECTIONS.iter().map(|s| s.items.len()).sum()
}

impl SysInspectUX {
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

        let max_label_w = MENU_SECTIONS.iter().flat_map(|s| s.items.iter()).map(|(label, _)| label.len()).max().unwrap_or(10);
        let max_item_w = max_label_w + 34;

        let title_segments = [" Actions on ".chars().count(), format!(" {host} ").chars().count()];
        let title_w = 1usize + title_segments.iter().sum::<usize>() + (title_segments.len().saturating_sub(1)) + 1usize;
        let inner_w = max_item_w.max(title_w) as u16;

        let section_headers = MENU_SECTIONS.len() as u16;
        let item_rows = total_menu_items() as u16;
        let inner_h = section_headers + item_rows + 2;

        let w = (inner_w + 2).min(parent.width.saturating_sub(8)).max(20);
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

        let hint_style = Style::default().fg(palette::PRIMARY);

        let mut row_y = inner.y;
        let mut flat_idx: usize = 0;

        for (si, section) in MENU_SECTIONS.iter().enumerate() {
            if row_y >= inner.bottom() {
                break;
            }
            if si > 0 && row_y < inner.bottom() {
                row_y += 1;
            }
            dashed_title(
                Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
                buf,
                section.title,
                palette::PROCESSING,
                palette::PRIMARY,
                palette::PROCESSING_DIMMED,
            );
            row_y += 1;

            for &(label, key) in section.items {
                if row_y >= inner.bottom() {
                    break;
                }
                let selected = flat_idx == self.minions_menu_sel;
                let item_style =
                    if selected { Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT) } else { Style::default().fg(palette::FG).bg(bg) };

                let hint = format!("^{key}");
                let padding = (inner.width as usize).saturating_sub(label.len() + 1 + hint.len()).saturating_sub(2); // one space on each side
                let line = format!(" {label}{}{hint} ", " ".repeat(padding));
                buf.set_string(inner.x, row_y, &line, item_style);

                // Re-paint just the key hint with its own style on top
                let hint_x = inner.x + (inner.width.saturating_sub(hint.len() as u16 + 2));
                let hint_sel_style = if selected { Style::default().fg(palette::BG_0).bg(palette::HIGHLIGHT) } else { hint_style };
                buf.set_string(hint_x, row_y, &hint, hint_sel_style);

                row_y += 1;
                flat_idx += 1;
            }
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
